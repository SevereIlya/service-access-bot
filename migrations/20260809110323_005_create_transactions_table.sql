CREATE TABLE transactions
(
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id          BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    amount           BIGINT      NOT NULL,
    type             TEXT        NOT NULL,
    status           TEXT        NOT NULL,
    payment_provider TEXT        NOT NULL,
    provider_tx_id   TEXT UNIQUE,
    payload          JSONB,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);