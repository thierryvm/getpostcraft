use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

pub mod accounts;
pub mod ai_usage;
pub mod history;
pub mod settings_db;

pub async fn init_pool() -> Result<SqlitePool, String> {
    let db_path = dirs::data_dir()
        .ok_or("Cannot resolve data dir")?
        .join("getpostcraft")
        .join("app.db");

    // Ensure directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.to_string_lossy()))
            .map_err(|e| e.to_string())?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|e| e.to_string())?;

    // Run migrations
    sqlx::migrate!("src/db/migrations")
        .run(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(pool)
}

// ── Migration regression tests (PR-C) ────────────────────────────────────────
//
// Every test runs against a fresh in-memory SQLite database, so they are fast,
// hermetic, and don't touch the user's real `app.db`. They exist to catch the
// failure modes that have actually bitten us:
//
//   - Migration 013 originally shipped with a FOREIGN KEY clause in its
//     ALTER TABLE — illegal in SQLite. Caught only after merging to main.
//     The "no FK on account_id" test below would have stopped that PR.
//   - Migration 011 carries a backfill that converts legacy `image_path`
//     values into the new `images` JSON array. A manual JSON construction
//     bug there produced corrupted rows on Windows paths in early testing.
//
// Adding a new migration? Add an end-state assertion here for every column,
// index, or backfill it introduces. Cheap up-front, expensive after a release
// (it ships, the user upgrades, the migration fails, the app refuses to start).
#[cfg(test)]
mod migration_tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Row, SqlitePool};

    /// Return a fresh in-memory pool with all migrations applied. The
    /// `:memory:` URL gives each test its own isolated DB. `max_connections=1`
    /// because in-memory SQLite databases are private to the connection that
    /// opened them — multiple connections each see an empty DB.
    async fn fresh_migrated_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect("migrations must apply cleanly");
        pool
    }

    /// List columns of a table via PRAGMA table_info.
    async fn table_columns(pool: &SqlitePool, table: &str) -> Vec<String> {
        let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(pool)
            .await
            .expect("PRAGMA table_info");
        rows.into_iter()
            .map(|r| r.get::<String, _>("name"))
            .collect()
    }

    /// Read foreign keys declared on a table via PRAGMA foreign_key_list.
    /// Each row maps a (from_column, references_table) pair.
    async fn table_foreign_keys(pool: &SqlitePool, table: &str) -> Vec<(String, String)> {
        let rows = sqlx::query(&format!("PRAGMA foreign_key_list({table})"))
            .fetch_all(pool)
            .await
            .expect("PRAGMA foreign_key_list");
        rows.into_iter()
            .map(|r| (r.get::<String, _>("from"), r.get::<String, _>("table")))
            .collect()
    }

    #[tokio::test]
    async fn all_migrations_apply_cleanly_to_a_fresh_db() {
        // Smoke test: just running fresh_migrated_pool already validates this,
        // but a named test makes the intent visible in CI output.
        let _pool = fresh_migrated_pool().await;
    }

    #[tokio::test]
    async fn final_schema_has_three_user_tables() {
        let pool = fresh_migrated_pool().await;
        for expected in ["accounts", "post_history", "settings"] {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?",
            )
            .bind(expected)
            .fetch_one(&pool)
            .await
            .expect("query master");
            assert_eq!(count, 1, "table `{expected}` must exist after migrations");
        }
    }

    #[tokio::test]
    async fn post_history_has_account_id_column_from_migration_013() {
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "post_history").await;
        assert!(
            cols.contains(&"account_id".to_string()),
            "migration 013 must add account_id column, got: {cols:?}"
        );
    }

    #[tokio::test]
    async fn post_history_has_no_foreign_keys_after_migration_013_hotfix() {
        // Regression guard for the migration 013 hotfix (PR #20). The
        // original migration shipped with `REFERENCES accounts(id) ON
        // DELETE SET NULL`, which SQLite rejects on ALTER TABLE ADD COLUMN.
        // The hotfix dropped the FK clause — if anyone reintroduces it,
        // this test fails before merge instead of in production.
        let pool = fresh_migrated_pool().await;
        let fks = table_foreign_keys(&pool, "post_history").await;
        assert!(
            fks.is_empty(),
            "post_history must have ZERO foreign keys (SQLite forbids ALTER \
             TABLE ADD COLUMN with FK). Found: {fks:?}"
        );
    }

    #[tokio::test]
    async fn accounts_has_visual_profile_from_migration_012() {
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "accounts").await;
        assert!(
            cols.contains(&"visual_profile".to_string()),
            "migration 012 must add visual_profile column, got: {cols:?}"
        );
    }

    #[tokio::test]
    async fn accounts_has_branding_colors_from_migration_009() {
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "accounts").await;
        for c in ["brand_color", "accent_color"] {
            assert!(
                cols.contains(&c.to_string()),
                "migration 009 must add {c} column, got: {cols:?}"
            );
        }
    }

    #[tokio::test]
    async fn accounts_has_product_truth_from_migration_007() {
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "accounts").await;
        assert!(
            cols.contains(&"product_truth".to_string()),
            "migration 007 must add product_truth column, got: {cols:?}"
        );
    }

    #[tokio::test]
    async fn post_history_carousel_columns_present_after_migration_011() {
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "post_history").await;
        // Migration 011 adds `images`; migration 004 added `image_path` and
        // `ig_media_id`. All three must coexist for the multi-image flow.
        for c in ["images", "image_path", "ig_media_id"] {
            assert!(
                cols.contains(&c.to_string()),
                "carousel column `{c}` missing, got: {cols:?}"
            );
        }
    }

    #[tokio::test]
    async fn migrations_are_idempotent_when_run_twice() {
        // Running `sqlx migrate run` after a successful run should be a no-op.
        // If a migration body is non-idempotent (e.g. INSERT without OR IGNORE),
        // the second run would fail or duplicate rows.
        let pool = fresh_migrated_pool().await;
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect("re-running migrations must be a no-op");
    }

    #[tokio::test]
    async fn insert_post_with_account_id_smoke() {
        // Functional smoke: after all migrations land, the schema actually
        // accepts the kind of row the publisher writes. Catches subtle issues
        // where a migration forgets to allow NULL on a new column, or the
        // ordering of columns broke a downstream INSERT.
        let pool = fresh_migrated_pool().await;

        // First insert an account so we have an id to bind.
        sqlx::query(
            "INSERT INTO accounts (provider, user_id, username, display_name, token_key) \
             VALUES ('instagram', '12345', 'test', NULL, 'instagram:12345')",
        )
        .execute(&pool)
        .await
        .expect("insert account");

        let account_id: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE user_id = '12345'")
            .fetch_one(&pool)
            .await
            .expect("fetch account id");

        // Now insert a post linking to that account — exercises every column
        // touched by migrations 001/002/004/011/013.
        sqlx::query(
            "INSERT INTO post_history \
                (network, caption, hashtags, status, created_at, image_path, images, account_id) \
             VALUES ('instagram', 'caption', '[\"tag\"]', 'draft', '2026-01-01T00:00:00Z', \
                     NULL, '[]', ?)",
        )
        .bind(account_id)
        .execute(&pool)
        .await
        .expect("insert post must succeed against post-013 schema");
    }

    #[tokio::test]
    async fn ai_usage_table_exists_after_migration_015() {
        // Cost tracker depends on this table being present from boot. The
        // smoke insert below confirms the schema accepts the shape Rust
        // writes — catches things like a NOT NULL on a column we'd want
        // to leave nullable.
        let pool = fresh_migrated_pool().await;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='ai_usage'",
        )
        .fetch_one(&pool)
        .await
        .expect("query master");
        assert_eq!(count, 1, "migration 015 must create the ai_usage table");

        // Functional smoke — the Rust insert path runs against this schema.
        sqlx::query(
            "INSERT INTO ai_usage (occurred_at, provider, model, action, input_tokens, output_tokens) \
             VALUES ('2026-05-08T12:00:00Z', 'openrouter', 'anthropic/claude-sonnet-4.6', \
                     'generate_content', 1234, 567)",
        )
        .execute(&pool)
        .await
        .expect("insert into ai_usage must succeed");
    }

    #[tokio::test]
    async fn accounts_has_token_expires_at_from_migration_014() {
        // PR-S6 introduces an OAuth token expiry column so the UI can show a
        // "expire dans X jours" badge before publishes start failing with a
        // silent 401. Nullable on purpose for legacy rows.
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "accounts").await;
        assert!(
            cols.contains(&"token_expires_at".to_string()),
            "migration 014 must add token_expires_at column, got: {cols:?}"
        );
    }

    #[tokio::test]
    async fn settings_seeded_with_default_provider_and_model() {
        // Migration 001 seeds active_provider + active_model so a fresh install
        // doesn't crash on first AI call. Migrations 005/006/008/010 update the
        // default model when OpenRouter rotates IDs. Final state must still have
        // both keys set to a non-empty value.
        let pool = fresh_migrated_pool().await;
        for key in ["active_provider", "active_model"] {
            let value: Option<String> =
                sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
                    .bind(key)
                    .fetch_optional(&pool)
                    .await
                    .expect("query settings");
            let v = value.unwrap_or_default();
            assert!(
                !v.trim().is_empty(),
                "settings[{key}] must be seeded to a non-empty default, got {v:?}"
            );
        }
    }
}
