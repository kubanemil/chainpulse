use tracing_subscriber::EnvFilter;

mod api;
mod db_writer;
mod indexer;

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

    // Create bounded MPSC channel for backpressure between indexers and DB writer
    let (tx, rx) = tokio::sync::mpsc::channel::<chainpulse_types::NormalizedEvent>(
        config.channel_buffer_size,
    );

    tracing::info!(
        channel_buffer_size = config.channel_buffer_size,
        batch_size = config.batch_size,
        flush_interval_ms = config.flush_interval_ms,
        "Bounded channel created"
    );

    // Spawn the DB writer task (owns the Receiver half and the pool for writes)
    let db_writer_handle = tokio::spawn(db_writer::run(
        rx,
        pool.clone(),
        config.batch_size,
        config.flush_interval_ms,
    ));

    // Spawn the API server task (read-only access to the database)
    let api_handle = tokio::spawn(api::start_server(
        config.server_host.clone(),
        config.server_port,
        pool.clone(),
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
        let handle = tokio::spawn(indexer::run_indexer(pool_clone, chain_config, tx_clone));
        handles.push(handle);
    }

    // Drop the original sender so the channel closes when all indexer tasks exit
    drop(tx);

    // Monitor all tasks: if any critical task exits, log and shut down
    tokio::select! {
        results = futures_util::future::join_all(handles) => {
            for result in results {
                match result {
                    Ok(Ok(())) => {
                        tracing::warn!("Indexer task exited normally (unexpected)");
                    }
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "Indexer task exited with error");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Indexer task panicked");
                    }
                }
            }

            // All indexer senders are dropped; wait for DB writer to finish draining
            match db_writer_handle.await {
                Ok(Ok(())) => {
                    tracing::info!("DB writer task exited normally");
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "DB writer task exited with error");
                }
                Err(e) => {
                    tracing::error!(error = %e, "DB writer task panicked");
                }
            }

            tracing::error!("All indexer tasks have exited, shutting down");
        }
        result = api_handle => {
            match result {
                Ok(Ok(())) => {
                    tracing::error!("API server exited unexpectedly");
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "API server exited with error");
                }
                Err(e) => {
                    tracing::error!(error = %e, "API server task panicked");
                }
            }
            tracing::error!("API server has exited, shutting down");
        }
    }

    std::process::exit(1);
}
