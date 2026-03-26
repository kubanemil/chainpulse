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

    indexer::run_indexer(&pool, &config).await;
}
