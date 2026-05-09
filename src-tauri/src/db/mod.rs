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

    // Snapshot the DB before we touch it — if a migration plants a column
    // change that bricks the schema, the user can roll back to the file the
    // app saw at startup. The daily auto-backup in ~/Documents covers
    // routine recovery; this snapshot covers the "I just upgraded and
    // things look weird" 5-minute window before the daily task fires.
    if let Err(e) = snapshot_db_before_migrate(&db_path) {
        log::warn!("db: pre-migration snapshot failed (non-fatal): {e}");
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

    // Heal stale checksums in _sqlx_migrations BEFORE running migrate!().run().
    //
    // sqlx records a checksum of every migration's bytes in _sqlx_migrations
    // and refuses to start if the current binary's embedded migration bytes
    // differ from what's recorded — even by a single line ending. We've seen
    // this fire when:
    //   - A user's DB was created on a build with CRLF endings, then upgraded
    //     to a build with LF endings (or vice versa).
    //   - File got cosmetically reformatted (indent changes) without
    //     functional change.
    // The schema is fine; only the recorded hash is out of sync. Healing
    // re-records the current bytes' hash so .run() sees no mismatch and
    // no-ops on the already-applied migrations. Brand-new installs bypass
    // this entirely (no _sqlx_migrations table yet).
    heal_migration_checksums(&pool)
        .await
        .map_err(|e| format!("Failed to heal migration checksums: {e}"))?;

    // Run migrations
    sqlx::migrate!("src/db/migrations")
        .run(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(pool)
}

/// Copy the live DB (if it exists) to `app.db.pre-migrate.bak` next to it
/// so a failed migration in the next step doesn't leave the user with no
/// rollback option. Cheap (single file copy in the data dir, megabytes,
/// not gigabytes) and best-effort: I/O failures here log a warning rather
/// than aborting startup, since the caller can still proceed and the
/// user has a daily auto-backup as a deeper safety net.
///
/// Only one snapshot is kept — overwritten each time the app starts.
/// That's intentional: this is a "right before the most recent upgrade"
/// safety net, not an archive. The auto-backup daily history covers
/// archival recovery.
fn snapshot_db_before_migrate(db_path: &std::path::Path) -> std::io::Result<()> {
    if !db_path.exists() {
        // First launch — no DB to snapshot yet.
        return Ok(());
    }
    let snapshot_path = db_path.with_extension("db.pre-migrate.bak");
    std::fs::copy(db_path, &snapshot_path)?;
    log::info!(
        "db: pre-migration snapshot saved to {}",
        snapshot_path.display()
    );
    Ok(())
}

/// Reconcile `_sqlx_migrations.checksum` with the bytes embedded in this
/// binary, for any migration row that's already present.
///
/// Returns the number of rows healed (0 when the DB is fresh or already
/// in sync). Errors propagate as `sqlx::Error` so the caller can wrap
/// them with a user-facing message.
async fn heal_migration_checksums(pool: &SqlitePool) -> Result<usize, sqlx::Error> {
    use sqlx::Row;

    // Bail on fresh installs — no _sqlx_migrations table means there's
    // nothing to heal and the regular migrate!().run() will create it.
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        return Ok(0);
    }

    let migrator = sqlx::migrate!("src/db/migrations");
    let mut healed = 0usize;

    for migration in migrator.iter() {
        let version: i64 = migration.version;
        let embedded_checksum: &[u8] = migration.checksum.as_ref();

        let stored = sqlx::query("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
            .bind(version)
            .fetch_optional(pool)
            .await?;
        let Some(row) = stored else {
            // Migration recorded as not-yet-applied — let .run() apply it.
            continue;
        };
        let stored_checksum: Vec<u8> = row.try_get("checksum")?;

        if stored_checksum != embedded_checksum {
            sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
                .bind(embedded_checksum)
                .bind(version)
                .execute(pool)
                .await?;
            healed += 1;
            log::warn!(
                "db: healed migration {version} checksum (stored != embedded — \
                 likely line-ending or whitespace drift between builds)"
            );
        }
    }

    Ok(healed)
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
    async fn post_history_has_published_url_from_migration_017() {
        // Migration 017 adds the deep-link URL column so the "Voir sur
        // {network}" button can target the actual post instead of the
        // account's profile feed (Instagram) or rebuild from URN
        // (LinkedIn). Nullable on purpose for legacy rows.
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "post_history").await;
        assert!(
            cols.contains(&"published_url".to_string()),
            "migration 017 must add published_url column, got: {cols:?}"
        );
    }

    #[tokio::test]
    async fn accounts_has_display_handle_from_migration_016() {
        // Migration 016 adds a brand-handle override so LinkedIn accounts
        // (whose `username` is the owner's full personal name) can render
        // visuals with a brand handle (e.g. @terminallearning) instead of
        // shipping the user's name on every slide. Nullable on purpose —
        // when NULL the renderer falls back to `username`.
        let pool = fresh_migrated_pool().await;
        let cols = table_columns(&pool, "accounts").await;
        assert!(
            cols.contains(&"display_handle".to_string()),
            "migration 016 must add display_handle column, got: {cols:?}"
        );
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
    async fn heal_migration_checksums_repairs_drifted_row() {
        // Reproduces the v0.3.5 → v0.3.6 startup crash: the user's _sqlx_migrations
        // had a checksum from build A, the new binary embeds bytes from build B
        // (CRLF/LF or whitespace drift), sqlx::migrate!().run() refused to start
        // with "migration N was previously applied but has been modified".
        //
        // Fix: heal_migration_checksums re-records the embedded checksum on
        // already-applied rows so .run() sees no mismatch and no-ops.
        let pool = fresh_migrated_pool().await;

        // Corrupt migration 1's checksum to simulate the build drift.
        let rows_corrupted =
            sqlx::query("UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = 1")
                .execute(&pool)
                .await
                .expect("corrupt checksum")
                .rows_affected();
        assert_eq!(rows_corrupted, 1, "must have a row at version 1 to corrupt");

        // Without the heal, sqlx::migrate!().run() must fail.
        let err = sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect_err("run must reject corrupted checksum");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("modified") || msg.contains("checksum"),
            "expected checksum-mismatch error, got: {msg}"
        );

        // Heal — should repair the one drifted row.
        let healed = super::heal_migration_checksums(&pool)
            .await
            .expect("heal must not error");
        assert_eq!(
            healed, 1,
            "exactly one row was corrupted, heal must repair 1"
        );

        // Now .run() must succeed (it's all already-applied → no-op).
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect("run must succeed after heal");
    }

    #[tokio::test]
    async fn heal_migration_checksums_is_noop_on_fresh_db() {
        // No _sqlx_migrations table yet (brand-new install). Heal must not
        // create the table or error — it just returns 0.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        let healed = super::heal_migration_checksums(&pool)
            .await
            .expect("heal on fresh db must not error");
        assert_eq!(healed, 0);

        // _sqlx_migrations table must still not exist (we didn't create it).
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
        )
        .fetch_one(&pool)
        .await
        .expect("query master");
        assert_eq!(exists, 0, "heal must not touch sqlite_master on fresh DB");
    }

    #[test]
    fn snapshot_before_migrate_copies_existing_db() {
        // Simulates the upgrade case: an app.db already exists, init_pool
        // must produce app.db.pre-migrate.bak so the user can roll back.
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("app.db");
        std::fs::write(&db_path, b"sqlite database bytes here").expect("write db");
        super::snapshot_db_before_migrate(&db_path).expect("snapshot must succeed");
        let snap = db_path.with_extension("db.pre-migrate.bak");
        assert!(snap.exists(), "snapshot file must be created");
        assert_eq!(
            std::fs::read(&snap).expect("read snap"),
            b"sqlite database bytes here",
            "snapshot bytes must match the source",
        );
    }

    #[test]
    fn snapshot_before_migrate_is_noop_on_first_launch() {
        // Brand-new install: no app.db yet. Snapshot must succeed silently
        // and not create anything (otherwise we'd litter empty files in
        // the data dir on every fresh startup).
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("app.db");
        super::snapshot_db_before_migrate(&db_path).expect("noop must succeed");
        assert!(!db_path.with_extension("db.pre-migrate.bak").exists());
    }

    #[test]
    fn snapshot_before_migrate_overwrites_previous_snapshot() {
        // We keep ONLY the most recent snapshot — older ones are overwritten
        // so the data dir doesn't accumulate copies after many startups.
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("app.db");
        let snap = db_path.with_extension("db.pre-migrate.bak");

        std::fs::write(&db_path, b"v1").expect("write db v1");
        super::snapshot_db_before_migrate(&db_path).expect("snap v1");
        assert_eq!(std::fs::read(&snap).expect("read snap"), b"v1");

        std::fs::write(&db_path, b"v2 newer bytes").expect("write db v2");
        super::snapshot_db_before_migrate(&db_path).expect("snap v2");
        assert_eq!(
            std::fs::read(&snap).expect("read snap"),
            b"v2 newer bytes",
            "snapshot must reflect the latest bytes",
        );
    }

    #[tokio::test]
    async fn heal_migration_checksums_skips_in_sync_rows() {
        // Sanity: when the embedded bytes already match the recorded checksum,
        // heal returns 0 and modifies nothing.
        let pool = fresh_migrated_pool().await;
        let healed = super::heal_migration_checksums(&pool)
            .await
            .expect("heal on in-sync DB must succeed");
        assert_eq!(
            healed, 0,
            "no rows should need healing on a freshly-migrated DB"
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
