use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder, PrometheusHandle};

const HEAD_BLOCK: &str = "chainpulse_head_block";
const INDEXED_BLOCK: &str = "chainpulse_indexed_block";
const LAG_BLOCKS: &str = "chainpulse_lag_blocks";
const REORGS: &str = "chainpulse_reorgs_total";
const REORG_DEPTH: &str = "chainpulse_reorg_depth_blocks";
const EVENTS: &str = "chainpulse_events_indexed_total";
const RPC_ERRORS: &str = "chainpulse_rpc_errors_total";
const WS_CLIENTS: &str = "chainpulse_ws_clients";

pub fn install() -> Result<PrometheusHandle, BuildError> {
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(REORG_DEPTH.to_owned()),
            &[1.0, 2.0, 3.0, 5.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0],
        )?
        .install_recorder()?;
    describe_gauge!(HEAD_BLOCK, "Latest block number reported by the node");
    describe_gauge!(INDEXED_BLOCK, "Block number of the indexer cursor");
    describe_gauge!(LAG_BLOCKS, "Blocks between the node head and the cursor");
    describe_counter!(REORGS, "Reorgs that rolled the cursor back");
    describe_histogram!(REORG_DEPTH, "Blocks rolled back per reorg");
    describe_counter!(EVENTS, "Events written, by chain and event type");
    describe_counter!(RPC_ERRORS, "Failed indexer steps caused by the node");
    describe_gauge!(WS_CLIENTS, "Connected WebSocket clients");
    Ok(handle)
}

pub fn head(chain: &str, head: u64) {
    gauge!(HEAD_BLOCK, "chain" => chain.to_owned()).set(head as f64);
}

pub fn progress(chain: &str, head: u64, indexed: u64) {
    gauge!(INDEXED_BLOCK, "chain" => chain.to_owned()).set(indexed as f64);
    gauge!(LAG_BLOCKS, "chain" => chain.to_owned()).set(head.saturating_sub(indexed) as f64);
}

pub fn reorg(chain: &str, depth: u64) {
    counter!(REORGS, "chain" => chain.to_owned()).increment(1);
    histogram!(REORG_DEPTH, "chain" => chain.to_owned()).record(depth as f64);
}

pub fn events_indexed(chain: &str, event_type: &str, count: u64) {
    counter!(EVENTS, "chain" => chain.to_owned(), "event_type" => event_type.to_owned())
        .increment(count);
}

pub fn rpc_error(chain: &str) {
    counter!(RPC_ERRORS, "chain" => chain.to_owned()).increment(1);
}

pub fn ws_client_connected() {
    gauge!(WS_CLIENTS).increment(1.0);
}

pub fn ws_client_disconnected() {
    gauge!(WS_CLIENTS).decrement(1.0);
}
