use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse environment variables: {0}")]
    Parse(#[from] envy::Error),
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_server_host() -> String {
    "0.0.0.0".to_string()
}

fn default_server_port() -> u16 {
    8080
}

fn default_channel_buffer_size() -> usize {
    1000
}

fn default_db_max_connections() -> u32 {
    10
}

fn default_batch_size() -> usize {
    100
}

fn default_flush_interval_ms() -> u64 {
    500
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub eth_rpc_ws_url: String,
    pub polygon_rpc_ws_url: String,

    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_server_host")]
    pub server_host: String,

    #[serde(default = "default_server_port")]
    pub server_port: u16,

    #[serde(default = "default_channel_buffer_size")]
    pub channel_buffer_size: usize,

    #[serde(default = "default_db_max_connections")]
    pub db_max_connections: u32,

    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    #[serde(default = "default_flush_interval_ms")]
    pub flush_interval_ms: u64,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();
        let config = envy::from_env::<Config>()?;
        Ok(config)
    }
}
