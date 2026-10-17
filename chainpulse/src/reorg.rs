//! Pure decisions about chain shape: what to fetch next, whether new blocks build on what is
//! stored, and where two forks meet.

use chainpulse_types::primitives::B256;
use chainpulse_types::{Block, BlockRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Nothing new. Check that the stored block at this height is still canonical.
    Verify(u64),
    /// The chain has not reached the configured start block yet.
    Wait,
    /// Index `from..=to`. Dense ranges lie inside the confirmation depth, so every header is
    /// fetched and linked; sparse ranges are final and only need the headers that carry logs.
    Fetch { from: u64, to: u64, dense: bool },
}

pub fn plan(
    cursor: Option<u64>,
    start_block: Option<u64>,
    head: u64,
    confirmations: u64,
    span: u64,
) -> Plan {
    let from = match cursor {
        Some(cursor) if cursor >= head => return Plan::Verify(head),
        Some(cursor) => cursor + 1,
        None => start_block.unwrap_or(head),
    };
    if from > head {
        return Plan::Wait;
    }
    let safe = head.saturating_sub(confirmations);
    let last = from.saturating_add(span.max(1) - 1);
    if from <= safe {
        Plan::Fetch {
            from,
            to: last.min(safe),
            dense: false,
        }
    } else {
        Plan::Fetch {
            from,
            to: last.min(head),
            dense: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Broken {
    /// The first new block does not build on the cursor: the cursor was reorged out.
    Cursor,
    /// Two adjacent fetched blocks are from different forks: the node switched forks mid-fetch.
    Within(u64),
}

/// Checks that `blocks` (ascending) extend `cursor` and each other wherever heights are adjacent.
pub fn check_links(cursor: Option<BlockRef>, blocks: &[Block]) -> Result<(), Broken> {
    if let (Some(cursor), Some(first)) = (cursor, blocks.first())
        && first.number == cursor.number + 1
        && first.parent_hash != cursor.hash
    {
        return Err(Broken::Cursor);
    }
    for pair in blocks.windows(2) {
        if let [parent, child] = pair
            && child.number == parent.number + 1
            && child.parent_hash != parent.hash
        {
            return Err(Broken::Within(child.number));
        }
    }
    Ok(())
}

/// `stored` is newest first and `canonical[i]` is the node's hash at `stored[i].number`, if it
/// has that block. The newest stored block the node still agrees with is the common ancestor.
pub fn common_ancestor(stored: &[BlockRef], canonical: &[Option<B256>]) -> Option<BlockRef> {
    stored
        .iter()
        .zip(canonical)
        .find(|(block, hash)| **hash == Some(block.hash))
        .map(|(block, _)| *block)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u8) -> B256 {
        B256::repeat_byte(n)
    }

    fn block(number: u64, hash_byte: u8, parent_byte: u8) -> Block {
        Block {
            number,
            hash: hash(hash_byte),
            parent_hash: hash(parent_byte),
            timestamp: number,
        }
    }

    fn at(number: u64, hash_byte: u8) -> BlockRef {
        BlockRef {
            number,
            hash: hash(hash_byte),
        }
    }

    #[test]
    fn backfills_final_blocks_sparsely_and_stops_at_the_confirmation_depth() {
        assert_eq!(
            plan(Some(100), None, 1_000, 64, 500),
            Plan::Fetch {
                from: 101,
                to: 600,
                dense: false
            }
        );
        assert_eq!(
            plan(Some(900), None, 1_000, 64, 500),
            Plan::Fetch {
                from: 901,
                to: 936,
                dense: false
            }
        );
    }

    #[test]
    fn indexes_unconfirmed_blocks_densely_up_to_the_head() {
        assert_eq!(
            plan(Some(936), None, 1_000, 64, 500),
            Plan::Fetch {
                from: 937,
                to: 1_000,
                dense: true
            }
        );
        assert_eq!(
            plan(Some(999), None, 1_000, 64, 500),
            Plan::Fetch {
                from: 1_000,
                to: 1_000,
                dense: true
            }
        );
    }

    #[test]
    fn verifies_the_tip_when_caught_up_or_when_the_node_is_behind() {
        assert_eq!(plan(Some(1_000), None, 1_000, 64, 500), Plan::Verify(1_000));
        assert_eq!(plan(Some(1_005), None, 1_000, 64, 500), Plan::Verify(1_000));
    }

    #[test]
    fn starts_at_the_configured_block_or_the_head() {
        assert_eq!(
            plan(None, Some(10), 1_000, 64, 500),
            Plan::Fetch {
                from: 10,
                to: 509,
                dense: false
            }
        );
        assert_eq!(
            plan(None, None, 1_000, 64, 500),
            Plan::Fetch {
                from: 1_000,
                to: 1_000,
                dense: true
            }
        );
        assert_eq!(plan(None, Some(2_000), 1_000, 64, 500), Plan::Wait);
    }

    #[test]
    fn with_zero_confirmations_every_block_is_final() {
        assert_eq!(
            plan(Some(999), None, 1_000, 0, 500),
            Plan::Fetch {
                from: 1_000,
                to: 1_000,
                dense: false
            }
        );
    }

    #[test]
    fn accepts_blocks_that_extend_the_cursor() {
        let blocks = [block(11, 11, 10), block(12, 12, 11)];

        assert_eq!(check_links(Some(at(10, 10)), &blocks), Ok(()));
        assert_eq!(check_links(None, &blocks), Ok(()));
    }

    #[test]
    fn detects_a_reorg_when_the_next_block_has_a_different_parent() {
        let blocks = [block(11, 11, 99)];

        assert_eq!(check_links(Some(at(10, 10)), &blocks), Err(Broken::Cursor));
    }

    #[test]
    fn detects_a_fork_switch_inside_a_dense_range() {
        let blocks = [block(11, 11, 10), block(12, 12, 77)];

        assert_eq!(
            check_links(Some(at(10, 10)), &blocks),
            Err(Broken::Within(12))
        );
    }

    #[test]
    fn does_not_link_blocks_with_gaps_between_them() {
        let blocks = [block(11, 11, 10), block(50, 50, 49), block(51, 51, 50)];

        assert_eq!(check_links(Some(at(10, 10)), &blocks), Ok(()));
    }

    #[test]
    fn finds_the_newest_stored_block_the_node_still_has() {
        let stored = [at(12, 12), at(11, 11), at(10, 10), at(9, 9)];
        let canonical = [
            Some(hash(0xa2)),
            Some(hash(0xa1)),
            Some(hash(10)),
            Some(hash(9)),
        ];

        assert_eq!(common_ancestor(&stored, &canonical), Some(at(10, 10)));
    }

    #[test]
    fn treats_blocks_the_node_no_longer_has_as_orphaned() {
        let stored = [at(12, 12), at(11, 11)];
        let canonical = [None, Some(hash(11))];

        assert_eq!(common_ancestor(&stored, &canonical), Some(at(11, 11)));
    }

    #[test]
    fn finds_no_ancestor_when_every_stored_block_was_replaced() {
        let stored = [at(12, 12), at(11, 11)];
        let canonical = [Some(hash(0xa2)), Some(hash(0xa1))];

        assert_eq!(common_ancestor(&stored, &canonical), None);
    }
}
