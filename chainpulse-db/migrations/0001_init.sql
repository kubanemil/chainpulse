-- One row per chain. Moves forward on commit and back only on a reorg rollback.
CREATE TABLE chain_cursors (
    chain_id        BIGINT PRIMARY KEY,
    block_number    BIGINT NOT NULL,
    block_hash      TEXT NOT NULL,
    confirmed_block BIGINT NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Recent canonical blocks, kept down to the confirmation depth, used to find the common ancestor in a reorg.
CREATE TABLE blocks (
    chain_id    BIGINT NOT NULL,
    number      BIGINT NOT NULL,
    hash        TEXT NOT NULL,
    parent_hash TEXT NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (chain_id, number)
);

CREATE TABLE events (
    id               BIGSERIAL PRIMARY KEY,
    chain_id         BIGINT NOT NULL,
    block_number     BIGINT NOT NULL,
    block_hash       TEXT NOT NULL,
    block_timestamp  TIMESTAMPTZ NOT NULL,
    log_index        INTEGER NOT NULL,
    tx_hash          TEXT NOT NULL,
    contract_address TEXT NOT NULL,
    event_type       TEXT NOT NULL,
    payload          JSONB NOT NULL,
    UNIQUE (chain_id, block_number, log_index)
);

CREATE INDEX events_contract_id ON events (contract_address, id);
CREATE INDEX events_type_id ON events (event_type, id);
