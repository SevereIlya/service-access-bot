CREATE TABLE users
(
    id                 BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY, -- Internal ID
    uuid               UUID        NOT NULL UNIQUE,                     -- External ID
    telegram_id        BIGINT      NOT NULL UNIQUE,
    username           TEXT,
    full_name          TEXT        NOT NULL,
    role               TEXT        NOT NULL DEFAULT 'user',
    frozen_base_price  BIGINT      NOT NULL,
    referral_code      TEXT        NOT NULL,
    subscription_token TEXT        NOT NULL UNIQUE,
    trial_used         BOOLEAN     NOT NULL DEFAULT FALSE,
    discount_percent   INT         NOT NULL DEFAULT 0 CHECK (discount_percent >= 0 AND discount_percent <= 100),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT users_referral_code_unique UNIQUE (referral_code),
    CONSTRAINT users_telegram_id_unique UNIQUE (telegram_id)
);
