-- Settings: active provider + model (replaces in-memory AppState)
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

-- Post history
CREATE TABLE IF NOT EXISTS post_history (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    network    TEXT    NOT NULL,
    caption    TEXT    NOT NULL,
    hashtags   TEXT    NOT NULL, -- JSON array
    status     TEXT    NOT NULL DEFAULT 'draft', -- draft | published | failed
    created_at TEXT    NOT NULL,
    published_at TEXT
);

-- Seed default provider settings
INSERT OR IGNORE INTO settings (key, value) VALUES
    ('active_provider', 'openrouter'),
    ('active_model',    'anthropic/claude-3-5-haiku');
