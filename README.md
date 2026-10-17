# ChainPulse

ChainPulse indexes contract events from EVM chains into Postgres and serves them over HTTP and a
WebSocket stream. You list chains and contracts in a TOML file. It follows each chain's head,
decodes ERC-20 transfers, Uniswap V2 and V3 swaps and pool creations, and Aave V3 supply,
withdraw and staking events, and keeps the database equal to the canonical chain when blocks get
reorganized. It is Rust (edition 2024) on alloy 1.8, axum 0.8, sqlx 0.8 and Postgres 17.

## Why it is built this way

An indexer is only worth running if its database matches the chain. Two things break that in
practice. Reorgs replace blocks the indexer already stored. Gaps open where a backfill hands over
to a live feed, or where a crash lands between writing events and writing progress.

So ChainPulse links every block it stores to the one before it by parent hash. A reorg shows up as
a broken link, not as quietly wrong rows. Blocks, events and the cursor commit in one transaction,
so a restart resumes from exactly the last committed block. Backfill and live indexing are the
same loop, so there is no handoff to get wrong.

## Architecture

```
  Ethereum RPC          Polygon RPC          one per [[chains]] entry
       |                     |
       v                     v
  +---------+           +---------+
  | indexer |           | indexer |          poll the head, fetch blocks and logs,
  +---------+           +---------+          check parent hashes, commit or roll back
       |   |                 |   |
       |   +--------+--------+   |           after each commit: events and reorgs,
       |            |            |           through an in-process broadcast
       |            v            |
       |    +---------------+    |
       |    | API (axum)    |    |
       |    | /events       |    |
       |    | /ws/events    |    |
       |    | /ready        |    |
       |    | /metrics      |    |
       |    +---------------+    |
       |            | reads      |
       v            v            v
  +-----------------------------------+
  | Postgres                          |     blocks, events and cursor
  | chain_cursors   blocks   events   |     in one transaction per step
  +-----------------------------------+
```

Each chain runs in its own task. If any task fails for good, the process stops all of them and
exits non-zero, so a supervisor restarts it. On SIGTERM the indexers stop, a step cut short rolls
back its transaction, and the API drains.

## How reorgs are handled

Every step asks the node for its head and picks the next range:

- Final blocks, at or below `head - confirmations`, come from one `eth_getLogs` over the range. The indexer fetches headers only for the first block, the last block and blocks that
  have logs, and checks each log's block hash against its header.
- Unconfirmed blocks, above `head - confirmations`, are fetched header by header. Each
  must name the previous one as its parent. Logs come per block through `eth_getLogs` with a
  `blockHash` filter, so they belong to exactly the header that was checked.

The first new block must name the stored cursor as its parent. When it does not, or when the node
now has a different block at the cursor's height, the indexer:

1. walks its stored blocks from newest to oldest and asks the node for the hash at each height,
2. takes the newest block both agree on as the common ancestor,
3. in one transaction deletes events and blocks above the ancestor and moves the cursor back,
4. sends a `reorg` message to WebSocket clients and records the depth in
   `chainpulse_reorg_depth_blocks`,
5. indexes forward again.

Logs marked `removed`, or whose block hash does not match the header, make the step retry.
Events at or below `head - confirmations` are served with `confirmed: true`. Block hashes are kept
down to that depth. A reorg deeper than it replaced blocks the config called final, so the
indexer stops with an error instead of serving data it can no longer verify.

## Layout

```
chainpulse/              binary and library
  src/indexer.rs         per-chain loop: fetch, link, commit, roll back
  src/reorg.rs           pure decisions: next range, link checks, common ancestor
  src/logs.rs            RPC logs to events, block hash checks
  src/api.rs  src/ws.rs  REST, readiness, WebSocket stream
  src/main.rs            startup, supervision, shutdown
  tests/                 integration tests against anvil and Postgres
chainpulse-config/       TOML config, env overrides, validation
chainpulse-db/           migration and every SQL query
chainpulse-types/        event ABIs, decoding, shared block and event types
```

## Development

You need Rust (tested on 1.99), Docker, and [Foundry](https://getfoundry.sh) for `anvil`.

```
./do test        unit tests
./do itest       integration tests: Postgres in Docker, a fresh anvil and database per test
./do check       rustfmt --check and clippy with warnings as errors
./do fmt         format the code
./do run         start Postgres in Docker and run the indexer on chainpulse.toml
./do dev         build and run Postgres and ChainPulse with Docker Compose
./do contracts   rebuild the test ERC-20 artifact with forge
```

The integration tests deploy a small ERC-20 on anvil, force reorgs with `anvil_snapshot` and
`anvil_revert` and with `anvil_reorg`, and check that the stored events and cursor equal the
canonical chain. CI runs `check`, `test`, `itest` and a Docker build on every pull request.

## API

```
GET /events          newest first; filters: chain_id, event_type, contract, from_block,
                     to_block, confirmed; paging: limit (max 1000), cursor
GET /events/{id}
GET /ws/events       send {"subscribe": {"chain_id": 1, "event_type": "Transfer"}}
GET /health          the process is up
GET /ready           200 when Postgres answers and every chain is within max_lag_blocks
GET /metrics         Prometheus: head, indexed block, lag, reorgs, reorg depth, events, RPC errors
```

A page returns `next_cursor`; pass it back as `cursor` for the next, older page. WebSocket clients
get `{"type": "event", ...}` with the same fields as `/events`, and
`{"type": "reorg", "chain_id": 1, "common_ancestor": 21000000, "depth": 2, "removed_events": 3}`,
which means every event above `common_ancestor` on that chain is gone.

## Configuration

`chainpulse.toml` indexes Ethereum and Polygon through public dRPC endpoints. Point
`CHAINPULSE_CONFIG` at another file to change that.

```toml
[database]
url = "postgres://chainpulse:chainpulse@localhost:5432/chainpulse"

[api]
listen = "0.0.0.0:8080"
max_ws_clients = 100

[[chains]]
id = 1
name = "ethereum"
rpc_url = "https://eth.drpc.org"   # http(s):// or ws(s)://
confirmations = 64                 # deeper blocks are final
start_block = 21000000             # omit to start at the head
poll_interval_ms = 2000
max_block_range = 500              # per eth_getLogs call; halves while the node refuses
max_lag_blocks = 5                 # /ready fails above this

[[chains.contracts]]
name = "USDC"
address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
events = ["Transfer"]
```

Known events: `Transfer`, `SwapV2`, `SwapV3`, `PairCreated`, `PoolCreated`, `Supply`, `Withdraw`,
`Staked`. Startup checks the whole file and lists every problem at once.

| Variable            | Effect                                                 |
| ------------------- | ------------------------------------------------------ |
| `DATABASE_URL`      | overrides `database.url`                               |
| `<NAME>_RPC_URL`    | overrides that chain's `rpc_url`, e.g. `ETHEREUM_RPC_URL` |
| `CHAINPULSE_CONFIG` | config path, `chainpulse.toml` by default              |
| `RUST_LOG`          | log filter, `info` by default                          |
| `LOG_FORMAT=json`   | JSON logs; the Docker image sets it                    |

The node has to support `eth_getLogs` with a `blockHash` filter (EIP-234). Every major client does,
but some free endpoints refuse it or cap the number of addresses per call.

## Decisions

| Choice | Not chosen | Why |
| --- | --- | --- |
| Poll the head, fetch by block | `eth_subscribe("logs")` | Nodes ignore `fromBlock` on a subscription, so events between the backfill and the subscribe are lost. A log feed also never reports empty blocks, so the cursor cannot advance or check hashes. Polling is one loop for backfill and live and works over HTTP. |
| Logs by `blockHash` near the head | One range query for the window | A range query can answer from a different fork than the headers fetched a moment earlier. A `blockHash` filter ties the logs to the header that was linked. |
| Range queries for final blocks | Per-block calls everywhere | Backfill would need a call per block. Final blocks are not expected to change, and log hashes are still checked against headers. |
| Each indexer writes its own steps | A shared writer fed by a channel | The cursor has to commit with its events. A channel in between splits them across transactions and needs its own back-pressure. Waiting on Postgres is the back-pressure. |
| Cursor moved by compare-and-set | `GREATEST` upsert | The cursor must move back on a reorg, which `GREATEST` refuses. Compare-and-set allows that and makes a second indexer on the same chain fail instead of interleaving writes. |
| Stop on a reorg deeper than `confirmations` | Roll back as far as needed | Those block hashes are pruned and their events were served as confirmed. Stopping surfaces the broken assumption. |
| Retry node errors, exit on database errors | Retry everything | RPC providers flake and recover. A broken database needs an operator, and restarts are safe because they resume from the committed cursor. |
| `confirmed` derived from the cursor row | A flag rewritten on each event | One row update per step instead of rewriting old events as the chain grows. |
| Keyset pages on `id` | `LIMIT`/`OFFSET` with a total | Offsets shift while events arrive, and `COUNT(*)` slows down as the table grows. |
| TOML with env overrides | Env vars only | Chains and contracts are nested lists. Only secrets come from the environment. |

## Roadmap

Milestone 2:

- A Solana source that streams program transactions through Yellowstone Geyser gRPC, with
  `logsSubscribe` as the fallback, and decodes instructions and events from Anchor IDLs. Slots
  and commitment levels take the place of blocks and confirmation depth.
- Arbitrum, Base and Optimism in the default config, using the node's `finalized` tag instead
  of a fixed depth where the chain has one.
- Event definitions loaded from ABI files in the config instead of the fixed decoder list.
- `newHeads` over WebSocket to wake the loop when a block lands, with polling still deciding
  what gets written.

## License

MIT
