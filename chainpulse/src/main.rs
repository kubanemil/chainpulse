#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use chainpulse::api;
use chainpulse::db_writer;
use chainpulse::indexer;

#[tokio::main]
async fn main() {
    let config = match chainpulse_config::Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .json()
        .init();

    // Install Prometheus metrics recorder before any metrics macros are used
    let prometheus_handle = match metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
    {
        Ok(handle) => handle,
        Err(e) => {
            tracing::error!(error = %e, "Failed to install Prometheus recorder");
            std::process::exit(1);
        }
    };

    tracing::info!(
        server_host = %config.server_host,
        server_port = config.server_port,
        "ChainPulse starting"
    );

    let pool = match chainpulse_db::create_pool(&config.database_url, config.db_max_connections).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!(error = %e, "Failed to connect to database");
            std::process::exit(1);
        }
    };

    if let Err(e) = chainpulse_db::run_migrations(&pool).await {
        tracing::error!(error = %e, "Failed to run migrations");
        std::process::exit(1);
    }

    tracing::info!("Database connected, migrations applied");

    // Create the cancellation token for coordinated shutdown
    let token = CancellationToken::new();

    // Spawn the signal handler task
    let signal_token = token.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal(signal_token).await;
    });

    // Build chain configs for all supported chains
    let chain_configs = vec![
        indexer::ChainConfig {
            name: "ethereum".to_string(),
            chain_id: 1,
            ws_url: config.eth_rpc_ws_url.clone(),
            contracts: chainpulse_types::default_eth_contracts(),
        },
        indexer::ChainConfig {
            name: "polygon".to_string(),
            chain_id: 137,
            ws_url: config.polygon_rpc_ws_url.clone(),
            contracts: chainpulse_types::default_polygon_contracts(),
        },
    ];

    // Create shared health state from chain configs
    let health_state = api::HealthState::new(
        chain_configs.iter().map(|c| (c.name.clone(), c.chain_id)).collect(),
    );

    // Create bounded MPSC channel for backpressure between indexers and DB writer
    let (tx, rx) = tokio::sync::mpsc::channel::<chainpulse_types::NormalizedEvent>(
        config.channel_buffer_size,
    );

    // Create broadcast channel for WebSocket event streaming
    let (broadcast_tx, _broadcast_rx) =
        tokio::sync::broadcast::channel::<chainpulse_types::NormalizedEvent>(
            config.ws_broadcast_capacity,
        );

    tracing::info!(
        channel_buffer_size = config.channel_buffer_size,
        batch_size = config.batch_size,
        flush_interval_ms = config.flush_interval_ms,
        ws_broadcast_capacity = config.ws_broadcast_capacity,
        "Bounded channel created"
    );

    // Spawn the DB writer task (owns the Receiver half and the pool for writes)
    let db_writer_handle = tokio::spawn(db_writer::run(
        rx,
        pool.clone(),
        config.batch_size,
        config.flush_interval_ms,
        broadcast_tx.clone(),
        token.clone(),
    ));

    // Spawn the API server task (read-only access to the database)
    let api_handle = tokio::spawn(api::start_server(
        config.server_host.clone(),
        config.server_port,
        pool.clone(),
        broadcast_tx,
        config.ws_max_connections,
        prometheus_handle,
        health_state.clone(),
        token.clone(),
    ));

    // Spawn one indexer task per chain
    let mut handles = Vec::new();
    for chain_config in chain_configs {
        tracing::info!(
            chain = %chain_config.name,
            chain_id = chain_config.chain_id,
            contracts = chain_config.contracts.len(),
            "Spawning indexer task"
        );
        let pool_clone = pool.clone();
        let tx_clone = tx.clone();
        let health_clone = health_state.clone();
        let token_clone = token.clone();
        let handle = tokio::spawn(indexer::run_indexer(pool_clone, chain_config, tx_clone, health_clone, token_clone));
        handles.push(handle);
    }

    // Drop the original sender so the channel closes when all indexer tasks exit
    drop(tx);

    // Wait for cancellation (triggered by signal handler)
    token.cancelled().await;
    tracing::info!("Shutdown initiated, waiting for tasks to complete");

    let shutdown_timeout = Duration::from_secs(config.shutdown_timeout_secs);

    // Wait for all tasks to complete with a timeout
    let shutdown_result = tokio::time::timeout(shutdown_timeout, async {
        // Wait for indexer tasks
        let indexer_results = futures_util::future::join_all(handles).await;
        for result in indexer_results {
            match result {
                Ok(Ok(())) => {
                    tracing::info!("Indexer task exited cleanly");
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "Indexer task exited with error");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Indexer task panicked");
                }
            }
        }
        tracing::info!("All indexer tasks stopped");

        // Wait for DB writer to finish draining
        match db_writer_handle.await {
            Ok(Ok(())) => {
                tracing::info!("DB writer task exited cleanly");
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "DB writer task exited with error");
            }
            Err(e) => {
                tracing::error!(error = %e, "DB writer task panicked");
            }
        }
        tracing::info!("DB writer stopped");

        // Wait for API server
        match api_handle.await {
            Ok(Ok(())) => {
                tracing::info!("API server exited cleanly");
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "API server exited with error");
            }
            Err(e) => {
                tracing::error!(error = %e, "API server task panicked");
            }
        }
        tracing::info!("API server stopped");
    })
    .await;

    match shutdown_result {
        Ok(()) => {
            tracing::info!("All tasks completed, closing database connections");
        }
        Err(_) => {
            tracing::warn!(
                timeout_secs = config.shutdown_timeout_secs,
                "Shutdown timed out, forcing exit"
            );
        }
    }

    // Close the database pool
    pool.close().await;
    tracing::info!("Database connections closed");

    tracing::info!("Shutdown complete");
}

/// Listen for SIGTERM and SIGINT. First signal cancels the token for graceful
/// shutdown. Second signal forces an immediate exit.
async fn wait_for_shutdown_signal(token: CancellationToken) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Failed to install SIGTERM handler");
                std::process::exit(1);
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Failed to install SIGINT handler");
                std::process::exit(1);
            }
        };

        // Wait for first signal
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, initiating graceful shutdown");
            }
            _ = sigint.recv() => {
                tracing::info!("Received SIGINT, initiating graceful shutdown");
            }
        }

        token.cancel();

        // Wait for second signal — force exit
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::warn!("Received second SIGTERM, forcing immediate exit");
            }
            _ = sigint.recv() => {
                tracing::warn!("Received second SIGINT, forcing immediate exit");
            }
        }

        std::process::exit(1);
    }

    #[cfg(not(unix))]
    {
        // On non-Unix platforms, use ctrl_c only
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to listen for Ctrl+C");
            std::process::exit(1);
        }
        tracing::info!("Received Ctrl+C, initiating graceful shutdown");
        token.cancel();

        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to listen for Ctrl+C");
            std::process::exit(1);
        }
        tracing::warn!("Received second Ctrl+C, forcing immediate exit");
        std::process::exit(1);
    }
}
