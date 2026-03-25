use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::eips::BlockNumberOrTag;
use futures_util::StreamExt;
use tracing_subscriber::EnvFilter;

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

    // ── WebSocket connection ─────────────────────────────────────────────
    let ws = WsConnect::new(&config.eth_rpc_ws_url);
    let provider = match ProviderBuilder::new().connect_ws(ws).await {
        Ok(provider) => provider,
        Err(e) => {
            tracing::error!(error = %e, "Failed to connect WebSocket to Ethereum");
            std::process::exit(1);
        }
    };

    tracing::info!("WebSocket connected to Ethereum");

    // ── Log subscription ─────────────────────────────────────────────────
    let contracts = chainpulse_types::default_contracts();
    let num_contracts = contracts.len();

    let filter = Filter::new()
        .address(contracts)
        .from_block(BlockNumberOrTag::Latest);

    let sub = match provider.subscribe_logs(&filter).await {
        Ok(sub) => sub,
        Err(e) => {
            tracing::error!(error = %e, "Failed to subscribe to logs");
            std::process::exit(1);
        }
    };

    let mut stream = sub.into_stream();

    tracing::info!(contract_count = num_contracts, "Subscribed to logs for {} contracts", num_contracts);

    // ── Event processing loop ────────────────────────────────────────────
    while let Some(log) = stream.next().await {
        if let Some(event) = chainpulse_types::decode_log(&log) {
            match chainpulse_db::insert_event(
                &pool,
                event.chain_id,
                event.block_number,
                event.log_index,
                &event.tx_hash,
                &event.contract_address,
                &event.event_type,
                &event.payload,
            )
            .await
            {
                Ok(_) => {
                    tracing::info!(
                        event_type = %event.event_type,
                        block_number = event.block_number,
                        contract_address = %event.contract_address,
                        "Event indexed"
                    );
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to write event to database");
                    std::process::exit(1);
                }
            }
        } else {
            tracing::debug!("Skipping unknown event");
        }
    }

    tracing::error!("WebSocket stream ended");
    std::process::exit(1);
}
