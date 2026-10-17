use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chainpulse_types::EventKind;
use chainpulse_types::primitives::Address;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Parse(#[from] toml::de::Error),
    #[error("invalid config:\n  - {}", .0.join("\n  - "))]
    Invalid(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub api: ApiConfig,
    pub chains: Vec<ChainConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default = "defaults::max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    #[serde(default = "defaults::listen")]
    pub listen: SocketAddr,
    #[serde(default = "defaults::max_ws_clients")]
    pub max_ws_clients: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainConfig {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub rpc_url: String,
    /// Blocks deeper than this below the head are final. A reorg deeper than this stops the indexer.
    pub confirmations: u64,
    /// First block to index on a fresh database. The chain head when unset.
    pub start_block: Option<u64>,
    #[serde(default = "defaults::poll_interval_ms")]
    pub poll_interval_ms: u64,
    #[serde(default = "defaults::max_block_range")]
    pub max_block_range: u64,
    /// `/ready` fails while the indexer is more than this many blocks behind the head.
    #[serde(default = "defaults::max_lag_blocks")]
    pub max_lag_blocks: u64,
    pub contracts: Vec<ContractConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractConfig {
    pub name: String,
    pub address: Address,
    pub events: Vec<EventKind>,
}

mod defaults {
    use std::net::{Ipv4Addr, SocketAddr};

    pub fn max_connections() -> u32 {
        10
    }
    pub fn listen() -> SocketAddr {
        (Ipv4Addr::UNSPECIFIED, 8080).into()
    }
    pub fn max_ws_clients() -> usize {
        100
    }
    pub fn poll_interval_ms() -> u64 {
        2_000
    }
    pub fn max_block_range() -> u64 {
        1_000
    }
    pub fn max_lag_blocks() -> u64 {
        10
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: defaults::max_connections(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            listen: defaults::listen(),
            max_ws_clients: defaults::max_ws_clients(),
        }
    }
}

impl ChainConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_interval_ms)
    }

    /// Env var that overrides `rpc_url`, so API keys stay out of the config file.
    pub fn rpc_url_env(&self) -> String {
        format!("{}_RPC_URL", self.name.to_uppercase())
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(&text, |key| std::env::var(key).ok())
    }

    pub fn parse(text: &str, env: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut config: Config = toml::from_str(text)?;
        let env = |key: &str| env(key).filter(|value| !value.is_empty());
        if let Some(url) = env("DATABASE_URL") {
            config.database.url = url;
        }
        for chain in &mut config.chains {
            if let Some(url) = env(&chain.rpc_url_env()) {
                chain.rpc_url = url;
            }
        }
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut problems = Vec::new();
        if self.database.url.is_empty() {
            problems.push("database.url is empty; set it or DATABASE_URL".to_owned());
        }
        if self.database.max_connections == 0 {
            problems.push("database.max_connections must be at least 1".to_owned());
        }
        if self.chains.is_empty() {
            problems.push("no [[chains]] configured".to_owned());
        }

        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for chain in &self.chains {
            let at = format!("chain `{}`", chain.name);
            if !is_identifier(&chain.name) {
                problems.push(format!("{at}: name must match [a-z][a-z0-9_]*"));
            }
            if !names.insert(&chain.name) {
                problems.push(format!("{at}: duplicate name"));
            }
            if !ids.insert(chain.id) {
                problems.push(format!("{at}: duplicate id {}", chain.id));
            }
            if i64::try_from(chain.id).is_err() {
                problems.push(format!(
                    "{at}: id {} does not fit in a signed 64-bit integer",
                    chain.id
                ));
            }
            if chain.rpc_url.is_empty() {
                problems.push(format!(
                    "{at}: rpc_url is empty; set it or {}",
                    chain.rpc_url_env()
                ));
            } else if !["http://", "https://", "ws://", "wss://"]
                .iter()
                .any(|scheme| chain.rpc_url.starts_with(scheme))
            {
                problems.push(format!(
                    "{at}: rpc_url must start with http(s):// or ws(s)://"
                ));
            }
            if chain.poll_interval_ms == 0 {
                problems.push(format!("{at}: poll_interval_ms must be at least 1"));
            }
            if chain.max_block_range == 0 {
                problems.push(format!("{at}: max_block_range must be at least 1"));
            }
            if chain.contracts.is_empty() {
                problems.push(format!("{at}: no contracts configured"));
            }
            let mut addresses = HashSet::new();
            for contract in &chain.contracts {
                if !addresses.insert(contract.address) {
                    problems.push(format!("{at}: contract {} listed twice", contract.address));
                }
                if contract.events.is_empty() {
                    problems.push(format!("{at}: contract `{}` has no events", contract.name));
                }
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(problems))
        }
    }
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
        [database]
        url = "postgres://localhost/chainpulse"

        [[chains]]
        id = 1
        name = "ethereum"
        rpc_url = "https://eth.example"
        confirmations = 64

        [[chains.contracts]]
        name = "USDC"
        address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        events = ["Transfer"]
    "#;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn problems(text: &str) -> Vec<String> {
        match Config::parse(text, no_env) {
            Err(ConfigError::Invalid(problems)) => problems,
            other => panic!("expected validation errors, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_minimal_config_with_defaults() {
        let config = Config::parse(VALID, no_env).unwrap();

        let chain = &config.chains[0];
        assert_eq!(chain.poll_interval(), Duration::from_secs(2));
        assert_eq!(chain.max_block_range, 1_000);
        assert_eq!(chain.start_block, None);
        assert_eq!(chain.contracts[0].events, vec![EventKind::Transfer]);
        assert_eq!(config.api.listen.port(), 8080);
    }

    #[test]
    fn env_vars_override_secrets() {
        let env = |key: &str| match key {
            "DATABASE_URL" => Some("postgres://prod/db".to_owned()),
            "ETHEREUM_RPC_URL" => Some("wss://eth.example/KEY".to_owned()),
            _ => None,
        };

        let config = Config::parse(VALID, env).unwrap();

        assert_eq!(config.database.url, "postgres://prod/db");
        assert_eq!(config.chains[0].rpc_url, "wss://eth.example/KEY");
    }

    #[test]
    fn empty_env_vars_do_not_override() {
        let env = |key: &str| (key == "ETHEREUM_RPC_URL").then(String::new);

        assert_eq!(
            Config::parse(VALID, env).unwrap().chains[0].rpc_url,
            "https://eth.example"
        );
    }

    #[test]
    fn the_shipped_config_is_valid() {
        let config = Config::parse(include_str!("../../chainpulse.toml"), no_env).unwrap();

        let names: Vec<&str> = config.chains.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["ethereum", "polygon"]);
    }

    #[test]
    fn reports_every_problem_at_once() {
        let text = VALID
            .replace("url = \"postgres://localhost/chainpulse\"", "")
            .replace("https://eth.example", "eth.example")
            .replace("[\"Transfer\"]", "[]");

        let problems = problems(&text);

        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].contains("DATABASE_URL"));
        assert!(problems[1].contains("rpc_url must start with"));
        assert!(problems[2].contains("has no events"));
    }

    #[test]
    fn rejects_duplicate_chains_and_contracts() {
        let contract = "\n[[chains.contracts]]\nname = \"again\"\naddress = \"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48\"\nevents = [\"Transfer\"]\n";
        let text = format!(
            "{VALID}{contract}{}",
            &VALID[VALID.find("[[chains]]").unwrap()..]
        );

        let problems = problems(&text);

        assert!(
            problems.iter().any(|p| p.contains("listed twice")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("duplicate name")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("duplicate id 1")),
            "{problems:?}"
        );
    }

    #[test]
    fn rejects_unknown_event_names_with_the_valid_list() {
        let err = Config::parse(&VALID.replace("\"Transfer\"", "\"Mint\""), no_env).unwrap_err();

        assert!(
            err.to_string()
                .contains("unknown event `Mint`, expected one of: Transfer"),
            "{err}"
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        let err =
            Config::parse(&VALID.replace("confirmations", "confirmation"), no_env).unwrap_err();

        assert!(matches!(err, ConfigError::Parse(_)), "{err}");
    }

    #[test]
    fn rejects_chain_names_that_cannot_be_env_vars() {
        let problems = problems(&VALID.replace("\"ethereum\"", "\"Eth-Mainnet\""));

        assert!(problems[0].contains("name must match"), "{problems:?}");
    }
}
