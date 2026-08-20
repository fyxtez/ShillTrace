-- Non-token addresses are kept outside token/tracking tables so wallet calls
-- never affect channel token statistics, Max X, or market polling workload.
CREATE TABLE IF NOT EXISTS wallets (
    id          BIGSERIAL PRIMARY KEY,
    chain_id    TEXT NOT NULL,
    address     TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS wallets_chain_address_unique
    ON wallets(chain_id, LOWER(address));

CREATE TABLE IF NOT EXISTS wallet_mentions (
    id                  BIGSERIAL PRIMARY KEY,
    wallet_id           BIGINT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    channel_id          BIGINT NOT NULL REFERENCES channels(telegram_id),
    telegram_message_id BIGINT NOT NULL REFERENCES telegram_messages(id),
    mentioned_at        TIMESTAMPTZ NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(wallet_id, telegram_message_id)
);

CREATE INDEX IF NOT EXISTS wallet_mentions_time_idx ON wallet_mentions(mentioned_at DESC);
