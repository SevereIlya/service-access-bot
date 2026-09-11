CREATE TABLE transactions
(
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id          BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    amount           BIGINT      NOT NULL CHECK (amount > 0),
    kind             TEXT        NOT NULL,
    status           TEXT        NOT NULL,
    payment_provider TEXT        NOT NULL,
    order_id         TEXT        NOT NULL UNIQUE,
    provider_tx_id   TEXT UNIQUE,
    payload          JSONB,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_transactions_user_id ON transactions (user_id) WHERE status = 'success';