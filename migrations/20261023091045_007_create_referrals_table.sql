CREATE TABLE referrals
(
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    referrer_id BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    referred_id BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL,
    CONSTRAINT referrals_referred_id_unique UNIQUE (referred_id),
    CONSTRAINT referrals_no_self_referral CHECK (referrer_id <> referred_id)
);
CREATE INDEX idx_referrals_referrer_id ON referrals (referrer_id);