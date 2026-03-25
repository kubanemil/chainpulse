CREATE TABLE events (
    id              BIGSERIAL PRIMARY KEY,
    chain_id        INTEGER NOT NULL,
    block_number    BIGINT NOT NULL,
    log_index       INTEGER NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL,
    contract_address VARCHAR(42) NOT NULL,
    event_type      VARCHAR(50) NOT NULL,
    payload         JSONB NOT NULL,
    block_timestamp TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(chain_id, block_number, log_index)
);

CREATE INDEX idx_events_chain_type ON events (chain_id, event_type);
CREATE INDEX idx_events_contract ON events (contract_address);
CREATE INDEX idx_events_block ON events (chain_id, block_number);

CREATE TABLE indexed_blocks (
    chain_id    INTEGER PRIMARY KEY,
    last_block  BIGINT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE tracked_contracts (
    id          SERIAL PRIMARY KEY,
    chain_id    INTEGER NOT NULL,
    address     VARCHAR(42) NOT NULL,
    name        VARCHAR(100),
    protocol    VARCHAR(100),
    event_types TEXT[] NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(chain_id, address)
);
