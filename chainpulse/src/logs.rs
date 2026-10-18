use std::collections::{BTreeSet, HashMap};

use alloy::primitives::{Address, B256};
use alloy::rpc::types::{Filter, Log};
use chainpulse_config::ContractConfig;
use chainpulse_types::{Block, Event, EventKind};

#[derive(Debug, Clone)]
pub struct Watched {
    kinds: HashMap<(Address, B256), EventKind>,
    filter: Filter,
}

impl Watched {
    pub fn new(contracts: &[ContractConfig]) -> Self {
        let kinds: HashMap<_, _> = contracts
            .iter()
            .flat_map(|c| {
                c.events
                    .iter()
                    .map(|kind| ((c.address, kind.topic0()), *kind))
            })
            .collect();
        let mut addresses: Vec<Address> = contracts.iter().map(|c| c.address).collect();
        let mut topics: Vec<B256> = kinds.keys().map(|(_, topic)| *topic).collect();
        addresses.sort();
        topics.sort();
        topics.dedup();
        let filter = Filter::new().address(addresses).event_signature(topics);
        Self { kinds, filter }
    }

    /// Matches every watched log, plus watched topics from other watched contracts; `assemble`
    /// drops those.
    pub fn filter(&self) -> Filter {
        self.filter.clone()
    }

    fn kind(&self, log: &Log) -> Option<EventKind> {
        let topic = log.topic0()?;
        self.kinds.get(&(log.address(), *topic)).copied()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Inconsistent(pub String);

/// Headers for the range ends link the cursor; others only supply timestamps the logs lack.
pub fn headers_needed(from: u64, to: u64, logs: &[Log]) -> BTreeSet<u64> {
    let mut numbers: BTreeSet<u64> = logs
        .iter()
        .filter(|log| log.block_timestamp.is_none())
        .filter_map(|log| log.block_number)
        .collect();
    numbers.extend([from, to]);
    numbers
}

/// Every log must agree on its block's hash with the fetched header and with other logs from
/// that block, so one response cannot mix forks.
pub fn assemble(
    chain_id: u64,
    blocks: &[Block],
    logs: &[Log],
    watched: &Watched,
) -> Result<Vec<Event>, Inconsistent> {
    let mut hashes: HashMap<u64, B256> = blocks.iter().map(|b| (b.number, b.hash)).collect();
    let timestamps: HashMap<u64, u64> = blocks.iter().map(|b| (b.number, b.timestamp)).collect();
    let mut events = Vec::new();
    for log in logs {
        if log.removed {
            return Err(Inconsistent("node returned a removed log".to_owned()));
        }
        let (Some(number), Some(block_hash), Some(log_index), Some(tx_hash)) = (
            log.block_number,
            log.block_hash,
            log.log_index,
            log.transaction_hash,
        ) else {
            return Err(Inconsistent(
                "node returned a log without a block position".to_owned(),
            ));
        };
        let expected = *hashes.entry(number).or_insert(block_hash);
        if expected != block_hash {
            return Err(Inconsistent(format!(
                "log in block {number} has block hash {block_hash}, expected {expected}"
            )));
        }
        let Some(kind) = watched.kind(log) else {
            continue;
        };
        let Some(block_timestamp) = timestamps.get(&number).copied().or(log.block_timestamp) else {
            return Err(Inconsistent(format!("no timestamp for block {number}")));
        };
        match kind.decode(&log.inner.data) {
            Ok(payload) => events.push(Event {
                chain_id,
                block_number: number,
                block_hash,
                block_timestamp,
                log_index,
                tx_hash,
                contract: log.address(),
                kind,
                payload,
            }),
            Err(error) => tracing::warn!(
                chain_id, block = number, log_index, event = %kind, %error,
                "skipping log that does not match its event signature"
            ),
        }
    }
    events.sort_by_key(|e| (e.block_number, e.log_index));
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{LogData, U256, address};
    use alloy::sol_types::SolEvent;
    use chainpulse_types::Transfer;

    const TOKEN: Address = address!("1111111111111111111111111111111111111111");
    const OTHER: Address = address!("2222222222222222222222222222222222222222");

    fn watched() -> Watched {
        Watched::new(&[
            ContractConfig {
                name: "token".into(),
                address: TOKEN,
                events: vec![EventKind::Transfer],
            },
            ContractConfig {
                name: "pool".into(),
                address: OTHER,
                events: vec![EventKind::SwapV2],
            },
        ])
    }

    fn header(number: u64) -> Block {
        Block {
            number,
            hash: B256::repeat_byte(number as u8),
            parent_hash: B256::repeat_byte(number as u8 - 1),
            timestamp: 1_700_000_000 + number,
        }
    }

    fn transfer_log(address: Address, block: &Block, log_index: u64) -> Log {
        let data = Transfer {
            from: TOKEN,
            to: OTHER,
            value: U256::from(log_index),
        }
        .encode_log_data();
        Log {
            inner: alloy::primitives::Log { address, data },
            block_hash: Some(block.hash),
            block_number: Some(block.number),
            block_timestamp: None,
            transaction_hash: Some(B256::repeat_byte(0xee)),
            transaction_index: Some(0),
            log_index: Some(log_index),
            removed: false,
        }
    }

    #[test]
    fn builds_events_with_the_block_timestamp_in_log_order() {
        let blocks = [header(5), header(6)];
        let logs = [
            transfer_log(TOKEN, &blocks[1], 0),
            transfer_log(TOKEN, &blocks[0], 3),
        ];

        let events = assemble(1, &blocks, &logs, &watched()).unwrap();

        let positions: Vec<_> = events
            .iter()
            .map(|e| (e.block_number, e.log_index))
            .collect();
        assert_eq!(positions, [(5, 3), (6, 0)]);
        assert_eq!(events[0].block_timestamp, 1_700_000_005);
        assert_eq!(events[0].payload["value"], "3");
    }

    #[test]
    fn skips_events_not_configured_for_that_contract() {
        let blocks = [header(5)];
        let logs = [transfer_log(OTHER, &blocks[0], 0)];

        assert!(assemble(1, &blocks, &logs, &watched()).unwrap().is_empty());
    }

    #[test]
    fn skips_logs_that_do_not_decode() {
        let blocks = [header(5)];
        let mut log = transfer_log(TOKEN, &blocks[0], 0);
        log.inner.data = LogData::new_unchecked(vec![Transfer::SIGNATURE_HASH], Vec::new().into());

        assert!(assemble(1, &blocks, &[log], &watched()).unwrap().is_empty());
    }

    #[test]
    fn rejects_removed_logs() {
        let blocks = [header(5)];
        let mut log = transfer_log(TOKEN, &blocks[0], 0);
        log.removed = true;

        assert!(assemble(1, &blocks, &[log], &watched()).is_err());
    }

    #[test]
    fn rejects_logs_from_a_different_fork_than_the_header() {
        let blocks = [header(5)];
        let mut log = transfer_log(TOKEN, &blocks[0], 0);
        log.block_hash = Some(B256::repeat_byte(0xab));

        let err = assemble(1, &blocks, &[log], &watched()).unwrap_err();

        assert!(err.0.contains("has block hash"), "{err}");
    }

    #[test]
    fn takes_the_timestamp_from_the_log_when_the_header_was_not_fetched() {
        let mut log = transfer_log(TOKEN, &header(7), 0);
        log.block_timestamp = Some(1_700_000_007);

        let events = assemble(1, &[header(5)], &[log], &watched()).unwrap();

        assert_eq!(events[0].block_timestamp, 1_700_000_007);
    }

    #[test]
    fn rejects_a_log_with_neither_header_nor_timestamp() {
        let log = transfer_log(TOKEN, &header(7), 0);

        let err = assemble(1, &[header(5)], &[log], &watched()).unwrap_err();

        assert!(err.0.contains("no timestamp for block 7"), "{err}");
    }

    #[test]
    fn rejects_logs_of_one_block_that_disagree_on_its_hash() {
        let mut first = transfer_log(TOKEN, &header(7), 0);
        let mut second = transfer_log(TOKEN, &header(7), 1);
        first.block_timestamp = Some(1);
        second.block_timestamp = Some(1);
        second.block_hash = Some(B256::repeat_byte(0xab));

        assert!(assemble(1, &[], &[first, second], &watched()).is_err());
    }

    #[test]
    fn fetches_headers_only_for_range_ends_when_logs_carry_timestamps() {
        let mut stamped = transfer_log(TOKEN, &header(7), 0);
        stamped.block_timestamp = Some(1);
        let unstamped = transfer_log(TOKEN, &header(8), 0);

        assert_eq!(headers_needed(5, 9, &[stamped]), BTreeSet::from([5, 9]));
        assert_eq!(
            headers_needed(5, 9, &[unstamped]),
            BTreeSet::from([5, 8, 9])
        );
    }
}
