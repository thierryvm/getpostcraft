-- Migration 018 — post_groups + post_history.group_id.
--
-- Why: the v1 Composer publishes ONE network at a time. To reach a real
-- audience on Instagram + LinkedIn the user has to redo the whole flow
-- twice — same brief, two AI calls done sequentially, two drafts that
-- can drift apart, and orphan rows when the user only finishes one. This
-- migration introduces "post groups" so a single Composer pass can
-- generate sibling drafts (one per network) bound by a shared group id.
--
-- Sibling-rows model (vs JSON column on post_history):
--   - Each network keeps a dedicated row in post_history with its own
--     status / scheduled_at / published_at / images / hashtags. Every
--     existing query (publish, dashboard, calendar, history) keeps
--     working unchanged — they were already mono-network and they
--     stay mono-network.
--   - The new `group_id` column is the only join point. NULL on legacy
--     rows and on rows generated through the single-network path, so
--     retrocompat is naturally preserved (no backfill needed).
--
-- Why no `REFERENCES post_groups(id)` clause: SQLite refuses
-- `ALTER TABLE ... ADD COLUMN ... REFERENCES ...` outright — the same
-- gotcha that bit migration 013 (see the
-- `post_history_has_no_foreign_keys_after_migration_013_hotfix`
-- regression test). A full table recreation would let us declare the
-- FK at the DB level, but it doubles the migration's blast radius (copy
-- + index rebuild + WAL flush) for what's effectively a soft join key.
-- Cascade-equivalent behaviour lives in the Rust `db::groups` module:
-- deleting a group clears `group_id` on the children before dropping
-- the parent row, so siblings survive as standalone drafts the user
-- can still publish or delete one by one.
--
-- The brief is stored on post_groups (not duplicated across siblings)
-- because it's the only attribute that's truly identical across the
-- group. Captions, hashtags, and images diverge per-network and live
-- on each sibling row.

CREATE TABLE IF NOT EXISTS post_groups (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    brief       TEXT    NOT NULL,
    created_at  TEXT    NOT NULL
);

ALTER TABLE post_history ADD COLUMN group_id INTEGER;

CREATE INDEX IF NOT EXISTS idx_post_history_group_id
    ON post_history(group_id);
