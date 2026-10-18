# chainpulse

chainpulse follows the chains and contracts listed in `chainpulse.toml`, decodes their events
into Postgres, and serves them over HTTP and a WebSocket. It handles ERC-20 transfers, Uniswap V2
and V3 swaps and pool creations, Aave V3 supplies and withdrawals, and stkAAVE stakes.

To stay correct through reorgs, the indexer keeps the hashes of recent blocks and checks that
each new block names the stored tip as its parent. When it doesn't, or the node now has a
different block at the tip's height, the indexer walks its stored blocks back until it finds one
the node still has. In one transaction it deletes everything above that block and moves the
cursor back to it, then indexes the new fork. WebSocket clients get a `reorg` message with that
block number: every event above it on that chain is gone.

Blocks at least `confirmations` deep count as final. They are read with one `eth_getLogs` per
range, and their events are served with `confirmed: true`. Closer to the head, every header is
fetched and linked to the one before it, and logs are fetched per block by hash, so they can't
come from a different fork than the header. A reorg deeper than `confirmations` stops the process
with an error instead of serving data it can no longer check. Events and the cursor commit
together, so a restart picks up at the last committed block.

The integration tests deploy a small ERC-20 on anvil, force reorgs with `anvil_revert` and
`anvil_reorg`, and check that Postgres matches the chain afterwards. `./do itest` runs them; it
needs Docker and [Foundry](https://getfoundry.sh).

## Running it

```sh
./do run    # Postgres in Docker, then the indexer on Ethereum and Polygon
curl 'localhost:8080/events?chain_id=1&event_type=Transfer&limit=5'
```

The shipped config uses keyless public RPC endpoints. `ETHEREUM_RPC_URL`, `POLYGON_RPC_URL` and
`DATABASE_URL` override it, and `./do dev` runs the whole stack in Docker Compose. The node has
to accept `eth_getLogs` with a `blockHash` filter (EIP-234), which some free endpoints refuse.

`/ws/events` streams new events and reorgs; send `{"subscribe": {"chain_id": 1}}` to narrow it.
`/ready` fails while a chain lags, and `/metrics` is for Prometheus. `./do` lists the other tasks.

MIT licensed.
