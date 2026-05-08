-- AI usage ledger — one row per AI API call so the user can see what their
-- BYOK spending looks like without leaving the app.
--
-- Why we store *tokens* and not *cost*:
--
--   1. Pricing changes over time. If we computed cost at insert time, an
--      OpenRouter price drop next month would silently undercount or
--      overcount past calls. Tokens are an immutable, vendor-agnostic
--      atom of measurement.
--   2. The user might switch the model that produced a call's input/output
--      ratio later (replays, A/B tests). Re-pricing the historical record
--      against today's prices stays meaningful.
--
-- Cost is computed at *query time* by joining tokens against a small
-- in-memory pricing table living in `network_rules.rs::MODEL_PRICING`.
-- Unknown models fall back to a conservative estimate so the UI always
-- has a number to show — flagged "estimated" in the breakdown.
--
-- Indexed on `occurred_at` because every UI query filters by date range
-- (this month, last 30 days, all-time).
CREATE TABLE IF NOT EXISTS ai_usage (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at   TEXT NOT NULL,
    provider      TEXT NOT NULL,
    model         TEXT NOT NULL,
    -- The Tauri command that produced the call. Lets the UI show
    -- "you spent X on carousel generation this month" — useful when one
    -- action is suddenly disproportionate.
    action        TEXT NOT NULL,
    input_tokens  INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_ai_usage_occurred_at ON ai_usage(occurred_at);
