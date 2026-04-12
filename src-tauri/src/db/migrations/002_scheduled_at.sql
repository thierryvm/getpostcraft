-- Add scheduling support to post_history
ALTER TABLE post_history ADD COLUMN scheduled_at TEXT;

CREATE INDEX IF NOT EXISTS idx_post_history_scheduled_at
    ON post_history(scheduled_at);
