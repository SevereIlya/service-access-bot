CREATE TABLE devices
(
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id       BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    fingerprint   TEXT        NOT NULL,
    name          TEXT        NOT NULL,
    status        TEXT        NOT NULL,
    hardware_id   TEXT,
    os_name       TEXT,
    os_version    TEXT,
    device_model  TEXT,
    user_agent    TEXT,
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at  TIMESTAMPTZ NOT NULL,
    CONSTRAINT devices_user_fingerprint_unique UNIQUE (user_id, fingerprint)
);