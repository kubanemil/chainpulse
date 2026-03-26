use tracing_subscriber::EnvFilter;

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
        let handle = tokio::spawn(indexer::run_indexer(pool_clone, chain_config));
        handles.push(handle);
    }

    // Wait for all handles; if any task exits (error or panic), log and exit
    let results = futures_util::future::join_all(handles).await;

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

    tracing::error!("All indexer tasks have exited, shutting down");
    std::process::exit(1);
}
