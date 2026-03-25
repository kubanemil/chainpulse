---
date: 2026-03-25
author: Claude Opus 4.6
status: approved
branch: main
---

# Plan: M0 — Project Foundation

## Overview

Set up the ChainPulse Cargo workspace with a modular crate structure, typed configuration from environment variables, PostgreSQL schema with sqlx migrations, and a DB connection pool. This phase produces a compilable, testable skeleton that all subsequent milestones build on.

## Architecture Decisions

- **Cargo workspace with multiple crates**: Separate concerns early — `chainpulse` (binary), `chainpulse-config`, `chainpulse-db`, `chainpulse-types`. Keeps compile times manageable and enforces module boundaries.
- **Flat binary crate**: `main.rs` just wires config + DB pool and runs. No business logic in the binary crate.
- **sqlx with offline mode**: Use `sqlx prepare` for CI builds without a live DB. Migrations live in `chainpulse-db/migrations/`.
- **Single `events` table with JSONB payload**: Event-type-specific fields stored as JSONB rather than separate tables per event type. Simpler schema, easier to extend with new event types.
- **Composite unique constraint**: `(chain_id, block_number, log_index)` on `events` table for idempotent upserts.

## Prerequisites

- [ ] Rust toolchain installed (rustup)
- [ ] PostgreSQL running locally (or via Docker)
- [ ] `sqlx-cli` installed (`cargo install sqlx-cli --features postgres`)

---

## Phase 1: Cargo Workspace + Crate Structure

### Goal

Create a compilable Cargo workspace with four crates and minimal boilerplate.

### Files to Create

```
Cargo.toml                      # Workspace root
chainpulse/Cargo.toml           # Binary crate
chainpulse/src/main.rs          # Entry point (just prints "ChainPulse starting")
chainpulse-config/Cargo.toml    # Config crate
chainpulse-config/src/lib.rs    # Empty lib
chainpulse-db/Cargo.toml        # Database crate
chainpulse-db/src/lib.rs        # Empty lib
chainpulse-types/Cargo.toml     # Shared types crate
chainpulse-types/src/lib.rs     # Empty lib
.gitignore                      # Rust + .env patterns
```

### Module Design

**Workspace layout**:

- `chainpulse` — binary, depends on all other crates
- `chainpulse-config` — configuration parsing, no other internal deps
- `chainpulse-types` — shared types (event enums, chain ID), no other internal deps
- `chainpulse-db` — database layer, depends on `chainpulse-types`

**Root `Cargo.toml`** workspace members: all four crates. Define shared dependency versions via `[workspace.dependencies]`:

- `tokio` (full features)
- `serde` + `serde_json`
- `sqlx` (postgres, runtime-tokio, tls-rustls)
- `tracing` + `tracing-subscriber`
- `dotenvy`
- `envy`
- `thiserror`
- `chrono` (serde feature)

### Success Criteria

- `cargo build` succeeds
- `cargo test` succeeds (no tests yet, but no errors)

---

## Phase 2: Configuration

### Goal

Typed configuration struct deserialized from environment variables with validation.

### Files to Modify

- `chainpulse-config/Cargo.toml` — add `serde`, `envy`, `dotenvy` deps
- `chainpulse-config/src/lib.rs` — config struct + loading function

### Files to Create

- `.env.example` — documented example of all env vars

### Module Design

**`chainpulse-config`**

```
Config {
    database_url: String
    eth_rpc_ws_url: String
    polygon_rpc_ws_url: String
    log_level: String              # default: "info"
    server_host: String            # default: "0.0.0.0"
    server_port: u16               # default: 8080
    channel_buffer_size: usize     # default: 1000
    db_max_connections: u32        # default: 10
    batch_size: usize              # default: 100
}
```

Key functions:

- `Config::load() -> Result<Config, ConfigError>` — calls `dotenvy::dotenv().ok()`, then `envy::from_env()`
- Derive `serde::Deserialize` with `#[serde(default)]` for fields with defaults

### Integration

- `chainpulse/src/main.rs` calls `Config::load()` at startup, exits with clear error message if required vars are missing

### Success Criteria

- `cargo build` succeeds
- Setting `DATABASE_URL` and RPC URLs in `.env`, running binary prints loaded config
- Missing required var → clear error message

---

## Phase 3: Database Schema + Migrations

### Goal

Create PostgreSQL tables for events, indexed block tracking, and tracked contracts.

### Files to Create

- `chainpulse-db/migrations/001_initial_schema.sql`

### Data Structures

**`events` table**:

```sql
id              BIGSERIAL PRIMARY KEY
chain_id        INTEGER NOT NULL
block_number    BIGINT NOT NULL
log_index       INTEGER NOT NULL
tx_hash         VARCHAR(66) NOT NULL
contract_address VARCHAR(42) NOT NULL
event_type      VARCHAR(50) NOT NULL
payload         JSONB NOT NULL
block_timestamp TIMESTAMPTZ
created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()

UNIQUE(chain_id, block_number, log_index)
```

Indexes:
- `idx_events_chain_type` on `(chain_id, event_type)`
- `idx_events_contract` on `(contract_address)`
- `idx_events_block` on `(chain_id, block_number)`

**`indexed_blocks` table**:

```sql
chain_id        INTEGER PRIMARY KEY
last_block      BIGINT NOT NULL
updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
```

**`tracked_contracts` table**:

```sql
id              SERIAL PRIMARY KEY
chain_id        INTEGER NOT NULL
address         VARCHAR(42) NOT NULL
name            VARCHAR(100)
protocol        VARCHAR(100)
event_types     TEXT[] NOT NULL
created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()

UNIQUE(chain_id, address)
```

### Success Criteria

- `sqlx migrate run` succeeds against a local PostgreSQL
- Tables exist with correct columns, constraints, and indexes
- `sqlx migrate run` is idempotent (running twice doesn't error)

---

## Phase 4: Database Connection + Pool

### Goal

Wire up sqlx PgPool creation, expose basic DB operations, and verify connectivity.

### Files to Modify

- `chainpulse-db/Cargo.toml` — add `sqlx`, `chainpulse-types`, `chainpulse-config` deps
- `chainpulse-db/src/lib.rs` — pool creation + basic queries
- `chainpulse/Cargo.toml` — add dep on `chainpulse-db`
- `chainpulse/src/main.rs` — wire config → DB pool, verify connection on startup

### Module Design

**`chainpulse-db`**

Key functions:

- `create_pool(database_url: &str, max_connections: u32) -> Result<PgPool>` — creates connection pool
- `run_migrations(pool: &PgPool) -> Result<()>` — runs embedded migrations at startup
- `get_last_indexed_block(pool: &PgPool, chain_id: i32) -> Result<Option<i64>>` — query last block
- `upsert_last_indexed_block(pool: &PgPool, chain_id: i32, block: i64) -> Result<()>` — update tracker

### Integration

`main.rs` startup sequence:

1. `Config::load()`
2. Initialize `tracing_subscriber` with env filter from `config.log_level`
3. `create_pool(&config.database_url, config.db_max_connections)`
4. `run_migrations(&pool)`
5. Log "ChainPulse started, DB connected, migrations applied"

### Success Criteria

- `cargo build` succeeds
- With a running PostgreSQL and valid `DATABASE_URL`, binary starts and logs successful connection
- Migrations run automatically on startup
- `get_last_indexed_block` returns `None` for a fresh DB

---

## Rollback Plan

This is M0 / greenfield — rollback is `git reset` to before the first commit.

## Open Questions

None — all decisions resolved for M0.

---

## Next Steps

If you approve this plan, proceed to implementation:

```
/pipe-implement docs/dev/chainpulse/m0-project-foundation/implementation-plan.md
```

If you want changes:

- Tell me what to revise and I'll update the plan
- Ask questions about specific phases or decisions
