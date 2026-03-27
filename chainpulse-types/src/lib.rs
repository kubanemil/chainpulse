#![deny(clippy::unwrap_used, clippy::expect_used)]

use alloy::sol;
use alloy::sol_types::SolEvent;
use serde::{Deserialize, Serialize};

// ── Top-level ERC-20 Transfer ──────────────────────────────────────────────

sol! {
    #[derive(Debug)]
    event Transfer(address indexed from, address indexed to, uint256 value);
}

// ── Uniswap V2 events ─────────────────────────────────────────────────────

pub mod uniswap_v2 {
    use alloy::sol;

    sol! {
        #[derive(Debug)]
        event Swap(
            address indexed sender,
            uint256 amount0In,
            uint256 amount1In,
            uint256 amount0Out,
            uint256 amount1Out,
            address indexed to
        );
    }

    sol! {
        #[derive(Debug)]
        event PairCreated(
            address indexed token0,
            address indexed token1,
            address pair,
            uint256
        );
    }
}

// ── Uniswap V3 events ─────────────────────────────────────────────────────

pub mod uniswap_v3 {
    use alloy::sol;

    sol! {
        #[derive(Debug)]
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );
    }

    sol! {
        #[derive(Debug)]
        event PoolCreated(
            address indexed token0,
            address indexed token1,
            uint24 indexed fee,
            int24 tickSpacing,
            address pool
        );
    }
}

// ── Aave V3 events ────────────────────────────────────────────────────────

pub mod aave {
    use alloy::sol;

    sol! {
        #[derive(Debug)]
        event Supply(
            address indexed reserve,
            address user,
            address indexed onBehalfOf,
            uint256 amount,
            uint16 indexed referralCode
        );
    }

    sol! {
        #[derive(Debug)]
        event Withdraw(
            address indexed reserve,
            address indexed user,
            address indexed to,
            uint256 amount
        );
    }
}

// ── stkAAVE events ────────────────────────────────────────────────────────

pub mod stkaave {
    use alloy::sol;

    sol! {
        #[derive(Debug)]
        event Staked(
            address indexed from,
            address indexed onBehalfOf,
            uint256 amount
        );
    }
}

// ── NormalizedEvent ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub event_type: String,
    pub contract_address: String,
    pub tx_hash: String,
    pub block_number: i64,
    pub log_index: i32,
    pub chain_id: i32,
    pub payload: serde_json::Value,
}

// ── decode_log ─────────────────────────────────────────────────────────────

/// Format an address as 0x-prefixed lowercase hex.
fn fmt_addr(addr: &alloy::primitives::Address) -> String {
    format!("{}", addr).to_lowercase()
}

/// Decode a raw Alloy log into a NormalizedEvent by matching topic0 against
/// all known event signature hashes.
pub fn decode_log(log: &alloy::rpc::types::Log, chain_id: i32) -> Option<NormalizedEvent> {
    let topic0 = log.topic0()?;

    let block_number = log.block_number.unwrap_or(0) as i64;
    let log_index = log.log_index.unwrap_or(0) as i32;
    let tx_hash = format!("{}", log.transaction_hash.unwrap_or_default());
    let contract_address = fmt_addr(&log.address());

    let (event_type, payload) = if *topic0 == Transfer::SIGNATURE_HASH {
        let d = Transfer::decode_log_data(&log.inner.data).ok()?;
        (
            "Transfer",
            serde_json::json!({
                "from": fmt_addr(&d.from),
                "to": fmt_addr(&d.to),
                "value": d.value.to_string(),
            }),
        )
    } else if *topic0 == uniswap_v2::Swap::SIGNATURE_HASH {
        let d = uniswap_v2::Swap::decode_log_data(&log.inner.data).ok()?;
        (
            "SwapV2",
            serde_json::json!({
                "sender": fmt_addr(&d.sender),
                "amount0_in": d.amount0In.to_string(),
                "amount1_in": d.amount1In.to_string(),
                "amount0_out": d.amount0Out.to_string(),
                "amount1_out": d.amount1Out.to_string(),
                "to": fmt_addr(&d.to),
            }),
        )
    } else if *topic0 == uniswap_v3::Swap::SIGNATURE_HASH {
        let d = uniswap_v3::Swap::decode_log_data(&log.inner.data).ok()?;
        let tick: i32 = d.tick.as_i32();
        (
            "SwapV3",
            serde_json::json!({
                "sender": fmt_addr(&d.sender),
                "recipient": fmt_addr(&d.recipient),
                "amount0": d.amount0.to_string(),
                "amount1": d.amount1.to_string(),
                "sqrt_price_x96": d.sqrtPriceX96.to_string(),
                "liquidity": d.liquidity.to_string(),
                "tick": tick,
            }),
        )
    } else if *topic0 == aave::Supply::SIGNATURE_HASH {
        let d = aave::Supply::decode_log_data(&log.inner.data).ok()?;
        let referral_code: u16 = d.referralCode;
        (
            "Supply",
            serde_json::json!({
                "reserve": fmt_addr(&d.reserve),
                "user": fmt_addr(&d.user),
                "on_behalf_of": fmt_addr(&d.onBehalfOf),
                "amount": d.amount.to_string(),
                "referral_code": referral_code,
            }),
        )
    } else if *topic0 == aave::Withdraw::SIGNATURE_HASH {
        let d = aave::Withdraw::decode_log_data(&log.inner.data).ok()?;
        (
            "Withdraw",
            serde_json::json!({
                "reserve": fmt_addr(&d.reserve),
                "user": fmt_addr(&d.user),
                "to": fmt_addr(&d.to),
                "amount": d.amount.to_string(),
            }),
        )
    } else if *topic0 == stkaave::Staked::SIGNATURE_HASH {
        let d = stkaave::Staked::decode_log_data(&log.inner.data).ok()?;
        (
            "Staked",
            serde_json::json!({
                "from": fmt_addr(&d.from),
                "on_behalf_of": fmt_addr(&d.onBehalfOf),
                "amount": d.amount.to_string(),
            }),
        )
    } else if *topic0 == uniswap_v2::PairCreated::SIGNATURE_HASH {
        let d = uniswap_v2::PairCreated::decode_log_data(&log.inner.data).ok()?;
        (
            "PairCreated",
            serde_json::json!({
                "token0": fmt_addr(&d.token0),
                "token1": fmt_addr(&d.token1),
                "pair": fmt_addr(&d.pair),
                "pair_index": d._3.to_string(),
            }),
        )
    } else if *topic0 == uniswap_v3::PoolCreated::SIGNATURE_HASH {
        let d = uniswap_v3::PoolCreated::decode_log_data(&log.inner.data).ok()?;
        let fee: u32 = d.fee.to::<u32>();
        let tick_spacing: i32 = d.tickSpacing.as_i32();
        (
            "PoolCreated",
            serde_json::json!({
                "token0": fmt_addr(&d.token0),
                "token1": fmt_addr(&d.token1),
                "fee": fee,
                "tick_spacing": tick_spacing,
                "pool": fmt_addr(&d.pool),
            }),
        )
    } else {
        return None;
    };

    Some(NormalizedEvent {
        event_type: event_type.to_string(),
        contract_address,
        tx_hash,
        block_number,
        log_index,
        chain_id,
        payload,
    })
}

// ── default_contracts ──────────────────────────────────────────────────────

/// Returns the hardcoded list of Ethereum mainnet contract addresses to track.
pub fn default_eth_contracts() -> Vec<alloy::primitives::Address> {
    use alloy::primitives::address;
    vec![
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // USDC
        address!("dAC17F958D2ee523a2206206994597C13D831ec7"), // USDT
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), // WETH
        address!("6B175474E89094C44Da98b954EedeAC495271d0F"), // DAI
        address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"), // Uniswap V2 Factory
        address!("1F98431c8aD98523631AE4a59f267346ea31F984"), // Uniswap V3 Factory
        address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"), // Aave V3 Pool
        address!("4da27a545c0c5B758a6BA100e3a049001de870f5"), // stkAAVE
    ]
}

/// Returns the hardcoded list of Polygon PoS contract addresses to track.
pub fn default_polygon_contracts() -> Vec<alloy::primitives::Address> {
    use alloy::primitives::address;
    vec![
        address!("2791Bca1f2de4661ED88A30C99A7a9449Aa84174"), // USDC (bridged)
        address!("3c499c542cEF5E3811e1192ce70d8cC03d5c3359"), // USDC (native)
        address!("c2132D05D31c914a87C6611C10748AEb04B58e8F"), // USDT
        address!("0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270"), // WMATIC
        address!("7ceB23fD6bC0adD59E62ac25578270cFf1b9f619"), // WETH
        address!("5757371414417b8C6CAad45bAeF941aBc7d3Ab32"), // QuickSwap Factory
        address!("794a61358D6845594F94dc1DB02A252b5b4814aD"), // Aave V3 Pool
    ]
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use alloy::primitives::{address, b256, Address, B256, LogData, U256};

    /// Helper to build a mock `alloy::rpc::types::Log`.
    fn mock_log(
        addr: Address,
        topics: Vec<B256>,
        data: Vec<u8>,
        block_number: u64,
        log_index: u64,
        tx_hash: B256,
    ) -> alloy::rpc::types::Log {
        let inner = alloy::primitives::Log {
            address: addr,
            data: LogData::new(topics, data.into()).unwrap(),
        };
        alloy::rpc::types::Log {
            inner,
            block_hash: Some(B256::ZERO),
            block_number: Some(block_number),
            block_timestamp: None,
            transaction_hash: Some(tx_hash),
            transaction_index: Some(0),
            log_index: Some(log_index),
            removed: false,
        }
    }

    #[test]
    fn test_decode_transfer() {
        use alloy::sol_types::SolEvent;

        let from = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let to = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let value = U256::from(1_000_000u64);

        // Encode the event log data
        let log_data = Transfer::encode_log_data(&Transfer { from, to, value });

        let log = mock_log(
            address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_000,
            5,
            b256!("1111111111111111111111111111111111111111111111111111111111111111"),
        );

        let event = decode_log(&log, 1).expect("should decode Transfer");
        assert_eq!(event.event_type, "Transfer");
        assert_eq!(event.block_number, 18_000_000);
        assert_eq!(event.log_index, 5);
        assert_eq!(event.chain_id, 1);
        assert_eq!(
            event.payload["from"],
            format!("{}", from).to_lowercase()
        );
        assert_eq!(event.payload["value"], "1000000");
    }

    #[test]
    fn test_decode_swap_v2() {
        use alloy::sol_types::SolEvent;

        let sender = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let to = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let evt = uniswap_v2::Swap {
            sender,
            amount0In: U256::from(100u64),
            amount1In: U256::ZERO,
            amount0Out: U256::ZERO,
            amount1Out: U256::from(200u64),
            to,
        };
        let log_data = uniswap_v2::Swap::encode_log_data(&evt);

        let log = mock_log(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_001,
            10,
            B256::ZERO,
        );

        let event = decode_log(&log, 1).expect("should decode SwapV2");
        assert_eq!(event.event_type, "SwapV2");
        assert_eq!(event.payload["amount0_in"], "100");
        assert_eq!(event.payload["amount1_out"], "200");
    }

    #[test]
    fn test_decode_swap_v3() {
        use alloy::primitives::{I256, U160};
        use alloy::sol_types::SolEvent;

        let sender = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let recipient = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let evt = uniswap_v3::Swap {
            sender,
            recipient,
            amount0: I256::try_from(500i64).unwrap(),
            amount1: I256::try_from(-300i64).unwrap(),
            sqrtPriceX96: U160::from(1_000_000u64),
            liquidity: 999u128,
            tick: 100i32.try_into().unwrap(),
        };
        let log_data = uniswap_v3::Swap::encode_log_data(&evt);

        let log = mock_log(
            address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_002,
            0,
            B256::ZERO,
        );

        let event = decode_log(&log, 1).expect("should decode SwapV3");
        assert_eq!(event.event_type, "SwapV3");
        assert_eq!(event.payload["amount0"], "500");
        assert_eq!(event.payload["amount1"], "-300");
        assert_eq!(event.payload["tick"], 100);
    }

    #[test]
    fn test_decode_supply() {
        use alloy::sol_types::SolEvent;

        let evt = aave::Supply {
            reserve: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            user: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            onBehalfOf: address!("cccccccccccccccccccccccccccccccccccccccc"),
            amount: U256::from(5000u64),
            referralCode: 42u16,
        };
        let log_data = aave::Supply::encode_log_data(&evt);

        let log = mock_log(
            address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_003,
            1,
            B256::ZERO,
        );

        let event = decode_log(&log, 1).expect("should decode Supply");
        assert_eq!(event.event_type, "Supply");
        assert_eq!(event.payload["amount"], "5000");
        assert_eq!(event.payload["referral_code"], 42);
    }

    #[test]
    fn test_decode_withdraw() {
        use alloy::sol_types::SolEvent;

        let evt = aave::Withdraw {
            reserve: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            user: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            to: address!("cccccccccccccccccccccccccccccccccccccccc"),
            amount: U256::from(7777u64),
        };
        let log_data = aave::Withdraw::encode_log_data(&evt);

        let log = mock_log(
            address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_004,
            2,
            B256::ZERO,
        );

        let event = decode_log(&log, 1).expect("should decode Withdraw");
        assert_eq!(event.event_type, "Withdraw");
        assert_eq!(event.payload["amount"], "7777");
    }

    #[test]
    fn test_decode_staked() {
        use alloy::sol_types::SolEvent;

        let evt = stkaave::Staked {
            from: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            onBehalfOf: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            amount: U256::from(9999u64),
        };
        let log_data = stkaave::Staked::encode_log_data(&evt);

        let log = mock_log(
            address!("4da27a545c0c5B758a6BA100e3a049001de870f5"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_005,
            3,
            B256::ZERO,
        );

        let event = decode_log(&log, 1).expect("should decode Staked");
        assert_eq!(event.event_type, "Staked");
        assert_eq!(event.payload["amount"], "9999");
    }

    #[test]
    fn test_decode_pair_created() {
        use alloy::sol_types::SolEvent;

        let evt = uniswap_v2::PairCreated {
            token0: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            token1: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            pair: address!("cccccccccccccccccccccccccccccccccccccccc"),
            _3: U256::from(12345u64),
        };
        let log_data = uniswap_v2::PairCreated::encode_log_data(&evt);

        let log = mock_log(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_006,
            0,
            B256::ZERO,
        );

        let event = decode_log(&log, 1).expect("should decode PairCreated");
        assert_eq!(event.event_type, "PairCreated");
        assert_eq!(event.payload["pair_index"], "12345");
    }

    #[test]
    fn test_decode_pool_created() {
        use alloy::sol_types::SolEvent;

        let evt = uniswap_v3::PoolCreated {
            token0: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            token1: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            fee: 3000u32.try_into().unwrap(),
            tickSpacing: 60i32.try_into().unwrap(),
            pool: address!("cccccccccccccccccccccccccccccccccccccccc"),
        };
        let log_data = uniswap_v3::PoolCreated::encode_log_data(&evt);

        let log = mock_log(
            address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            18_000_007,
            1,
            B256::ZERO,
        );

        let event = decode_log(&log, 1).expect("should decode PoolCreated");
        assert_eq!(event.event_type, "PoolCreated");
        assert_eq!(event.payload["fee"], 3000);
        assert_eq!(event.payload["tick_spacing"], 60);
    }

    #[test]
    fn test_unknown_topic_returns_none() {
        let unknown_topic = b256!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
        let log = mock_log(
            address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            vec![unknown_topic],
            vec![],
            18_000_000,
            0,
            B256::ZERO,
        );

        assert!(decode_log(&log, 1).is_none());
    }

    #[test]
    fn test_malformed_data_returns_none() {
        // Correct topic0 for Transfer but garbage data
        let log = mock_log(
            address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            vec![Transfer::SIGNATURE_HASH, B256::ZERO, B256::ZERO],
            vec![0xde, 0xad], // too short to be a valid U256
            18_000_000,
            0,
            B256::ZERO,
        );

        assert!(decode_log(&log, 1).is_none());
    }

    #[test]
    fn test_no_topics_returns_none() {
        let inner = alloy::primitives::Log {
            address: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            data: LogData::new(vec![], vec![].into()).unwrap(),
        };
        let log = alloy::rpc::types::Log {
            inner,
            block_hash: None,
            block_number: None,
            block_timestamp: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        };

        assert!(decode_log(&log, 1).is_none());
    }

    #[test]
    fn test_default_eth_contracts_count() {
        let contracts = default_eth_contracts();
        assert_eq!(contracts.len(), 8);
    }

    #[test]
    fn test_default_eth_contracts_contains_usdc() {
        let contracts = default_eth_contracts();
        let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        assert!(contracts.contains(&usdc));
    }

    #[test]
    fn test_default_polygon_contracts_count() {
        let contracts = default_polygon_contracts();
        assert_eq!(contracts.len(), 7);
    }

    #[test]
    fn test_default_polygon_contracts_contains_usdc_bridged() {
        let contracts = default_polygon_contracts();
        let usdc_bridged = address!("2791Bca1f2de4661ED88A30C99A7a9449Aa84174");
        assert!(contracts.contains(&usdc_bridged));
    }

    #[test]
    fn test_decode_log_chain_id_137() {
        use alloy::sol_types::SolEvent;

        let from = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let to = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let value = U256::from(1_000_000u64);

        let log_data = Transfer::encode_log_data(&Transfer { from, to, value });

        let log = mock_log(
            address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            log_data.topics().to_vec(),
            log_data.data.to_vec(),
            50_000_000,
            2,
            b256!("2222222222222222222222222222222222222222222222222222222222222222"),
        );

        let event = decode_log(&log, 137).expect("should decode Transfer with chain_id 137");
        assert_eq!(event.chain_id, 137);
        assert_eq!(event.event_type, "Transfer");
    }
}
