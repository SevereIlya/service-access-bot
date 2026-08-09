CREATE TABLE vpn_connections
(
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id    BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    node_id    BIGINT      NOT NULL REFERENCES nodes (id) ON DELETE RESTRICT,

    -- Sync status (whether the user has been added to the panel on THIS server)
    is_synced  BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (user_id, node_id)
);