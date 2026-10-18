use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, anyhow};
use chainpulse::api::{self, AppState};
use chainpulse::indexer::Indexer;
use chainpulse::node::Node;
use chainpulse::telemetry;
use chainpulse_config::Config;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::broadcast;
use tokio::task::{JoinError, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

const FEED_CAPACITY: usize = 4_096;
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);
const API_STATEMENT_TIMEOUT: Duration = Duration::from_secs(10);

type TaskResult = (String, anyhow::Result<()>);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let path = std::env::var_os("CHAINPULSE_CONFIG")
        .map_or_else(|| PathBuf::from("chainpulse.toml"), PathBuf::from);
    let config =
        Config::load(&path).with_context(|| format!("loading config from {}", path.display()))?;
    let metrics = telemetry::install().context("installing the metrics recorder")?;

    // Separate pools so slow API queries cannot starve the indexers of connections.
    let indexer_slots = u32::try_from(config.chains.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let indexer_pool = chainpulse_db::connect(&config.database.url, indexer_slots, None)
        .await
        .context("connecting to Postgres")?;
    let api_pool = chainpulse_db::connect(
        &config.database.url,
        config.database.max_connections,
        Some(API_STATEMENT_TIMEOUT),
    )
    .await
    .context("connecting to Postgres")?;
    chainpulse_db::migrate(&indexer_pool)
        .await
        .context("running migrations")?;

    let shutdown = CancellationToken::new();
    let (feed, _) = broadcast::channel(FEED_CAPACITY);
    let mut tasks: JoinSet<TaskResult> = JoinSet::new();
    let mut statuses = Vec::new();

    for chain in config.chains {
        let name = chain.name.clone();
        let node = Node::connect(&chain.rpc_url, chain.rpc_timeout())
            .await
            .with_context(|| format!("connecting to the {name} node"))?;
        let indexer = Indexer::new(chain, node, indexer_pool.clone(), feed.clone()).await?;
        statuses.push(indexer.status());
        tracing::info!(chain = %name, cursor = ?indexer.cursor().map(|c| c.number), "starting indexer");
        let token = shutdown.clone();
        tasks.spawn(async move {
            (
                format!("{name} indexer"),
                indexer.run(token).await.map_err(Into::into),
            )
        });
    }

    let listener = TcpListener::bind(config.api.listen)
        .await
        .with_context(|| format!("binding {}", config.api.listen))?;
    tracing::info!(listen = %config.api.listen, "serving API");
    let state = AppState::new(
        api_pool.clone(),
        feed,
        statuses,
        metrics,
        config.api.max_ws_clients,
        shutdown.clone(),
    );
    tasks.spawn(async move {
        (
            "API server".to_owned(),
            api::serve(listener, state).await.map_err(Into::into),
        )
    });

    let mut sigterm = signal(SignalKind::terminate()).context("installing the SIGTERM handler")?;
    let failure = tokio::select! {
        _ = sigterm.recv() => None,
        _ = tokio::signal::ctrl_c() => None,
        Some(joined) = tasks.join_next() => Some(task_failure(joined)),
    };
    match &failure {
        None => tracing::info!("shutting down"),
        Some(error) => tracing::error!(
            error = format!("{error:#}"),
            "shutting down after a fatal error"
        ),
    }

    shutdown.cancel();
    let drained = tokio::time::timeout(SHUTDOWN_GRACE, async {
        while let Some(joined) = tasks.join_next().await {
            if let Ok((task, Err(error))) = joined {
                tracing::error!(
                    task,
                    error = format!("{error:#}"),
                    "task failed while stopping"
                );
            }
        }
    })
    .await;
    if drained.is_err() {
        tracing::warn!("tasks did not stop within {SHUTDOWN_GRACE:?}, aborting them");
        tasks.shutdown().await;
    }
    indexer_pool.close().await;
    api_pool.close().await;

    failure.map_or(Ok(()), Err)
}

/// Every task runs until shutdown, so any task finishing first is a failure.
fn task_failure(joined: Result<TaskResult, JoinError>) -> anyhow::Error {
    match joined {
        Ok((task, Ok(()))) => anyhow!("{task} stopped unexpectedly"),
        Ok((task, Err(error))) => error.context(format!("{task} failed")),
        Err(error) => anyhow!("task panicked: {error}"),
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let logs = tracing_subscriber::fmt().with_env_filter(filter);
    if std::env::var("LOG_FORMAT").is_ok_and(|format| format == "json") {
        logs.json().init();
    } else {
        logs.init();
    }
}
