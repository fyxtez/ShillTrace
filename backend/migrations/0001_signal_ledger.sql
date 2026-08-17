CREATE TABLE channels (
    telegram_id       BIGINT PRIMARY KEY,
    name              TEXT NOT NULL,
    kind              TEXT NOT NULL,
    is_ignored        BOOLEAN NOT NULL DEFAULT TRUE,
    has_photo         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE telegram_messages (
    id                  BIGSERIAL PRIMARY KEY,
    telegram_message_id BIGINT NOT NULL,
    channel_id          BIGINT NOT NULL REFERENCES channels(telegram_id),
    body                TEXT NOT NULL,
    sent_at             TIMESTAMPTZ NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(channel_id, telegram_message_id)
);

CREATE TABLE tokens (
    id                  BIGSERIAL PRIMARY KEY,
    chain_id            TEXT,
    contract_address    TEXT NOT NULL,
    symbol              TEXT,
    name                TEXT,
    image_url           TEXT,
    pair_address        TEXT,
    current_market_cap  DOUBLE PRECISION,
    market_status       TEXT NOT NULL DEFAULT 'unavailable',
    last_market_error   TEXT,
    last_market_at      TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX tokens_chain_contract_unique
    ON tokens(COALESCE(chain_id, 'unknown'), LOWER(contract_address));

CREATE TABLE tracking_periods (
    id                    BIGSERIAL PRIMARY KEY,
    token_id              BIGINT NOT NULL REFERENCES tokens(id),
    started_at            TIMESTAMPTZ NOT NULL,
    stopped_at            TIMESTAMPTZ,
    highest_market_cap    DOUBLE PRECISION,
    status                TEXT NOT NULL DEFAULT 'active',
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX one_active_period_per_token
    ON tracking_periods(token_id) WHERE status = 'active';

CREATE TABLE shills (
    id                    BIGSERIAL PRIMARY KEY,
    tracking_period_id    BIGINT NOT NULL REFERENCES tracking_periods(id),
    token_id              BIGINT NOT NULL REFERENCES tokens(id),
    channel_id            BIGINT NOT NULL REFERENCES channels(telegram_id),
    first_message_id      BIGINT NOT NULL REFERENCES telegram_messages(id),
    shilled_at            TIMESTAMPTZ NOT NULL,
    initial_market_cap    DOUBLE PRECISION,
    max_market_cap        DOUBLE PRECISION,
    market_status         TEXT NOT NULL DEFAULT 'unavailable',
    seen_at               TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tracking_period_id, channel_id)
);

CREATE TABLE shill_messages (
    shill_id              BIGINT NOT NULL REFERENCES shills(id) ON DELETE CASCADE,
    telegram_message_id   BIGINT NOT NULL REFERENCES telegram_messages(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(shill_id, telegram_message_id)
);

CREATE TABLE market_cap_samples (
    token_id              BIGINT NOT NULL REFERENCES tokens(id),
    tracking_period_id    BIGINT NOT NULL REFERENCES tracking_periods(id),
    market_cap            DOUBLE PRECISION NOT NULL,
    recorded_at           TIMESTAMPTZ NOT NULL,
    PRIMARY KEY(token_id, tracking_period_id, recorded_at)
);

CREATE INDEX shills_unseen_idx ON shills(shilled_at DESC) WHERE seen_at IS NULL;
CREATE INDEX samples_period_time_idx ON market_cap_samples(tracking_period_id, recorded_at DESC);
CREATE INDEX channels_ignored_idx ON channels(is_ignored, kind);

