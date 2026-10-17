use std::fmt;
use std::str::FromStr;

use alloy::primitives::{Address, B256, LogData};
use alloy::sol;
use alloy::sol_types::SolEvent;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};

pub use alloy::primitives;

sol! {
    #[derive(Debug)]
    event Transfer(address indexed from, address indexed to, uint256 value);
}

mod uniswap_v2 {
    alloy::sol! {
        #[derive(Debug)]
        event Swap(
            address indexed sender,
            uint256 amount0In,
            uint256 amount1In,
            uint256 amount0Out,
            uint256 amount1Out,
            address indexed to
        );

        #[derive(Debug)]
        event PairCreated(address indexed token0, address indexed token1, address pair, uint256);
    }
}

mod uniswap_v3 {
    alloy::sol! {
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

mod aave_v3 {
    alloy::sol! {
        #[derive(Debug)]
        event Supply(
            address indexed reserve,
            address user,
            address indexed onBehalfOf,
            uint256 amount,
            uint16 indexed referralCode
        );

        #[derive(Debug)]
        event Withdraw(address indexed reserve, address indexed user, address indexed to, uint256 amount);

        #[derive(Debug)]
        event Staked(address indexed from, address indexed onBehalfOf, uint256 amount);
    }
}

/// An event ChainPulse knows how to decode. The name is what config files and the API use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EventKind {
    Transfer,
    SwapV2,
    SwapV3,
    PairCreated,
    PoolCreated,
    Supply,
    Withdraw,
    Staked,
}

impl EventKind {
    pub const ALL: [EventKind; 8] = [
        Self::Transfer,
        Self::SwapV2,
        Self::SwapV3,
        Self::PairCreated,
        Self::PoolCreated,
        Self::Supply,
        Self::Withdraw,
        Self::Staked,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Transfer => "Transfer",
            Self::SwapV2 => "SwapV2",
            Self::SwapV3 => "SwapV3",
            Self::PairCreated => "PairCreated",
            Self::PoolCreated => "PoolCreated",
            Self::Supply => "Supply",
            Self::Withdraw => "Withdraw",
            Self::Staked => "Staked",
        }
    }

    pub const fn topic0(self) -> B256 {
        match self {
            Self::Transfer => Transfer::SIGNATURE_HASH,
            Self::SwapV2 => uniswap_v2::Swap::SIGNATURE_HASH,
            Self::SwapV3 => uniswap_v3::Swap::SIGNATURE_HASH,
            Self::PairCreated => uniswap_v2::PairCreated::SIGNATURE_HASH,
            Self::PoolCreated => uniswap_v3::PoolCreated::SIGNATURE_HASH,
            Self::Supply => aave_v3::Supply::SIGNATURE_HASH,
            Self::Withdraw => aave_v3::Withdraw::SIGNATURE_HASH,
            Self::Staked => aave_v3::Staked::SIGNATURE_HASH,
        }
    }

    /// Decodes the log into a JSON payload. Integers are strings so no precision is lost.
    pub fn decode(self, log: &LogData) -> alloy::sol_types::Result<Value> {
        let payload = match self {
            Self::Transfer => {
                let e = Transfer::decode_log_data(log)?;
                json!({ "from": hex(&e.from), "to": hex(&e.to), "value": e.value.to_string() })
            }
            Self::SwapV2 => {
                let e = uniswap_v2::Swap::decode_log_data(log)?;
                json!({
                    "sender": hex(&e.sender),
                    "to": hex(&e.to),
                    "amount0_in": e.amount0In.to_string(),
                    "amount1_in": e.amount1In.to_string(),
                    "amount0_out": e.amount0Out.to_string(),
                    "amount1_out": e.amount1Out.to_string(),
                })
            }
            Self::SwapV3 => {
                let e = uniswap_v3::Swap::decode_log_data(log)?;
                json!({
                    "sender": hex(&e.sender),
                    "recipient": hex(&e.recipient),
                    "amount0": e.amount0.to_string(),
                    "amount1": e.amount1.to_string(),
                    "sqrt_price_x96": e.sqrtPriceX96.to_string(),
                    "liquidity": e.liquidity.to_string(),
                    "tick": e.tick.as_i32(),
                })
            }
            Self::PairCreated => {
                let e = uniswap_v2::PairCreated::decode_log_data(log)?;
                json!({
                    "token0": hex(&e.token0),
                    "token1": hex(&e.token1),
                    "pair": hex(&e.pair),
                    "pair_index": e._3.to_string(),
                })
            }
            Self::PoolCreated => {
                let e = uniswap_v3::PoolCreated::decode_log_data(log)?;
                json!({
                    "token0": hex(&e.token0),
                    "token1": hex(&e.token1),
                    "fee": e.fee.to::<u32>(),
                    "tick_spacing": e.tickSpacing.as_i32(),
                    "pool": hex(&e.pool),
                })
            }
            Self::Supply => {
                let e = aave_v3::Supply::decode_log_data(log)?;
                json!({
                    "reserve": hex(&e.reserve),
                    "user": hex(&e.user),
                    "on_behalf_of": hex(&e.onBehalfOf),
                    "amount": e.amount.to_string(),
                    "referral_code": e.referralCode,
                })
            }
            Self::Withdraw => {
                let e = aave_v3::Withdraw::decode_log_data(log)?;
                json!({
                    "reserve": hex(&e.reserve),
                    "user": hex(&e.user),
                    "to": hex(&e.to),
                    "amount": e.amount.to_string(),
                })
            }
            Self::Staked => {
                let e = aave_v3::Staked::decode_log_data(log)?;
                json!({
                    "from": hex(&e.from),
                    "on_behalf_of": hex(&e.onBehalfOf),
                    "amount": e.amount.to_string(),
                })
            }
        };
        Ok(payload)
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEventKind(pub String);

impl fmt::Display for UnknownEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let known: Vec<&str> = EventKind::ALL.iter().map(|k| k.name()).collect();
        write!(
            f,
            "unknown event `{}`, expected one of: {}",
            self.0,
            known.join(", ")
        )
    }
}

impl std::error::Error for UnknownEventKind {}

impl FromStr for EventKind {
    type Err = UnknownEventKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name() == s)
            .ok_or_else(|| UnknownEventKind(s.to_owned()))
    }
}

impl Serialize for EventKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}

/// Lowercase `0x` hex, the one address format stored and served everywhere.
pub fn hex(address: &Address) -> String {
    format!("{address:#x}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockRef {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub timestamp: u64,
}

impl Block {
    pub fn to_ref(&self) -> BlockRef {
        BlockRef {
            number: self.number,
            hash: self.hash,
        }
    }
}

/// A decoded log from a block the indexer has verified against the chain.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: B256,
    pub block_timestamp: u64,
    pub log_index: u64,
    pub tx_hash: B256,
    pub contract: Address,
    pub kind: EventKind,
    pub payload: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{I256, U160, U256, address, aliases::I24, aliases::U24};
    use std::collections::HashSet;

    const A: Address = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    const B: Address = address!("BBbBbBBbbBbbbbBBbBbbBBbBBBbbBBbbbbbbBbBb");
    const C: Address = address!("cccccccccccccccccccccccccccccccccccccccc");

    #[test]
    fn decodes_erc20_transfer_with_lowercase_addresses() {
        let log = Transfer {
            from: A,
            to: B,
            value: U256::from(1_000_000u64),
        }
        .encode_log_data();

        let payload = EventKind::Transfer.decode(&log).unwrap();

        assert_eq!(
            payload,
            json!({
                "from": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "to": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "value": "1000000",
            })
        );
    }

    #[test]
    fn decodes_uniswap_v2_swap() {
        let log = uniswap_v2::Swap {
            sender: A,
            amount0In: U256::from(100u64),
            amount1In: U256::ZERO,
            amount0Out: U256::ZERO,
            amount1Out: U256::from(200u64),
            to: B,
        }
        .encode_log_data();

        let payload = EventKind::SwapV2.decode(&log).unwrap();

        assert_eq!(payload["amount0_in"], "100");
        assert_eq!(payload["amount1_out"], "200");
    }

    #[test]
    fn decodes_uniswap_v3_swap_with_signed_amounts() {
        let log = uniswap_v3::Swap {
            sender: A,
            recipient: B,
            amount0: I256::try_from(500i64).unwrap(),
            amount1: I256::try_from(-300i64).unwrap(),
            sqrtPriceX96: U160::from(1_000_000u64),
            liquidity: 999,
            tick: I24::try_from(-100i32).unwrap(),
        }
        .encode_log_data();

        let payload = EventKind::SwapV3.decode(&log).unwrap();

        assert_eq!(payload["amount0"], "500");
        assert_eq!(payload["amount1"], "-300");
        assert_eq!(payload["tick"], -100);
    }

    #[test]
    fn decodes_pool_and_pair_creation() {
        let pool = uniswap_v3::PoolCreated {
            token0: A,
            token1: B,
            fee: U24::from(3000u32),
            tickSpacing: I24::try_from(60i32).unwrap(),
            pool: C,
        }
        .encode_log_data();
        let pair = uniswap_v2::PairCreated {
            token0: A,
            token1: B,
            pair: C,
            _3: U256::from(7u64),
        }
        .encode_log_data();

        assert_eq!(EventKind::PoolCreated.decode(&pool).unwrap()["fee"], 3000);
        assert_eq!(
            EventKind::PairCreated.decode(&pair).unwrap()["pair_index"],
            "7"
        );
    }

    #[test]
    fn decodes_aave_events() {
        let supply = aave_v3::Supply {
            reserve: A,
            user: B,
            onBehalfOf: C,
            amount: U256::from(5000u64),
            referralCode: 42,
        }
        .encode_log_data();
        let withdraw = aave_v3::Withdraw {
            reserve: A,
            user: B,
            to: C,
            amount: U256::from(7u64),
        }
        .encode_log_data();
        let staked = aave_v3::Staked {
            from: A,
            onBehalfOf: B,
            amount: U256::from(9u64),
        }
        .encode_log_data();

        assert_eq!(
            EventKind::Supply.decode(&supply).unwrap()["referral_code"],
            42
        );
        assert_eq!(
            EventKind::Withdraw.decode(&withdraw).unwrap()["amount"],
            "7"
        );
        assert_eq!(EventKind::Staked.decode(&staked).unwrap()["amount"], "9");
    }

    #[test]
    fn rejects_log_with_malformed_data() {
        let log = LogData::new_unchecked(
            vec![Transfer::SIGNATURE_HASH, B256::ZERO, B256::ZERO],
            vec![0xde, 0xad].into(),
        );

        assert!(EventKind::Transfer.decode(&log).is_err());
    }

    #[test]
    fn rejects_erc721_transfer_with_extra_indexed_topic() {
        let log = LogData::new_unchecked(
            vec![Transfer::SIGNATURE_HASH, B256::ZERO, B256::ZERO, B256::ZERO],
            Vec::new().into(),
        );

        assert!(EventKind::Transfer.decode(&log).is_err());
    }

    #[test]
    fn every_kind_has_a_distinct_topic() {
        let topics: HashSet<B256> = EventKind::ALL.iter().map(|k| k.topic0()).collect();

        assert_eq!(topics.len(), EventKind::ALL.len());
    }

    #[test]
    fn names_round_trip_and_unknown_names_list_the_valid_ones() {
        for kind in EventKind::ALL {
            assert_eq!(kind.name().parse::<EventKind>(), Ok(kind));
        }

        let err = "transfer".parse::<EventKind>().unwrap_err().to_string();

        assert!(err.contains("unknown event `transfer`"));
        assert!(err.contains("Transfer, SwapV2"));
    }
}
