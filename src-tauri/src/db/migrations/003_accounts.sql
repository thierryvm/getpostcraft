-- OAuth connected accounts
CREATE TABLE IF NOT EXISTS accounts (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    provider     TEXT NOT NULL,           -- "instagram"
    user_id      TEXT NOT NULL,           -- provider-assigned user ID
    username     TEXT NOT NULL,
    display_name TEXT,
    token_key    TEXT NOT NULL,           -- key into oauth_tokens.json (never the token itself)
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(provider, user_id)
);
