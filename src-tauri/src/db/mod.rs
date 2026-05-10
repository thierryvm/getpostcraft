use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

pub mod accounts;
pub mod ai_usage;
pub mod groups;
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

/// Number of pre-migration snapshots kept on disk. Three balances disk cost
/// (a `.db` is typically 1–5 MB so the cap is single-digit MB) against
/// recovery breathing room: a user can mis-launch the app twice after a
/// botched migration and still have the original pre-failure copy on disk.
const MAX_PRE_MIGRATE_SNAPSHOTS: usize = 3;

const SNAPSHOT_SUFFIX: &str = ".bak";

/// Copy the live DB (if it exists) into a timestamped `<name>.pre-migrate-{ts}.bak`
/// next to it so a failed migration in the next step doesn't leave the user
/// with no rollback option. Cheap (single file copy in the data dir, MB not
/// GB) and best-effort: I/O failures log a warning rather than aborting
/// startup, since the caller can still proceed and the daily auto-backup
/// remains as a deeper safety net.
///
/// Up to `MAX_PRE_MIGRATE_SNAPSHOTS` snapshots are retained. Older ones are
/// pruned in the same call. The previous behaviour kept exactly one
/// (overwritten on every boot) — that left users with one window of
/// recovery and no margin if the app was restarted after the failure
/// before the user noticed something was wrong.
fn snapshot_db_before_migrate(db_path: &std::path::Path) -> std::io::Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3f").to_string();
    snapshot_db_before_migrate_with(db_path, &timestamp, MAX_PRE_MIGRATE_SNAPSHOTS)
}

/// Test-friendly core. The production entry-point feeds the current UTC
/// instant; tests inject a deterministic timestamp so we can reason about
/// rotation order without hitting a real clock.
fn snapshot_db_before_migrate_with(
    db_path: &std::path::Path,
    timestamp: &str,
    keep: usize,
) -> std::io::Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    // Hard-fail rather than fall back silently: a `db_path` without a parent
    // would land snapshots in the process CWD, and one without a stem would
    // collide on every snapshot. Both indicate a misconfigured caller — we
    // want the boot to fail loudly so the bug surfaces in init logs instead
    // of producing orphan files in unexpected locations.
    let parent = db_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("db_path {} has no parent directory", db_path.display()),
        )
    })?;
    let stem = db_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("db_path {} has no usable stem", db_path.display()),
            )
        })?;

    // Naming MUST match the previous `db_path.with_extension("db.pre-migrate.bak")`
    // semantic so deployments whose DB file isn't `app.db` (e.g. `notes.sqlite3`)
    // keep their legacy snapshots discoverable by the cleanup below. Deriving
    // from the stem gives us:
    //   - `app.db`         → `app.db.pre-migrate-{ts}.bak`        (legacy: `app.db.pre-migrate.bak`)
    //   - `notes.sqlite3`  → `notes.db.pre-migrate-{ts}.bak`      (legacy: `notes.db.pre-migrate.bak`)
    // The `-{ts}` separator (rather than `.{ts}`) avoids a `.with_extension`
    // collision with the legacy unrotated file and keeps glob matching trivial.
    let prefix = format!("{stem}.db.pre-migrate-");
    let snapshot_path = parent.join(format!("{prefix}{timestamp}{SNAPSHOT_SUFFIX}"));
    std::fs::copy(db_path, &snapshot_path)?;
    log::info!(
        "db: pre-migration snapshot saved to {}",
        snapshot_path.display()
    );

    // One-shot migration: pre-rotation builds wrote a single
    // `{stem}.db.pre-migrate.bak` and overwrote it on every boot. After we've
    // produced at least one timestamped snapshot, the legacy file is
    // redundant — and inflates the rotation accounting since it has no
    // timestamp to sort by. Best-effort cleanup, never fatal.
    let legacy = parent.join(format!("{stem}.db.pre-migrate.bak"));
    if legacy.exists() {
        if let Err(e) = std::fs::remove_file(&legacy) {
            log::warn!(
                "db: could not remove legacy snapshot {}: {e}",
                legacy.display()
            );
        }
    }

    if let Err(e) = prune_pre_migrate_snapshots(parent, &prefix, keep) {
        log::warn!("db: snapshot rotation cleanup failed (non-fatal): {e}");
    }

    Ok(())
}

/// Delete all but the `keep` newest snapshots that match `<prefix>*<SUFFIX>`
/// in `dir`. The timestamp embedded in the filename is fixed-width
/// `%Y%m%dT%H%M%S%3f`, so a lexicographic sort matches chronological order
/// without parsing.
fn prune_pre_migrate_snapshots(
    dir: &std::path::Path,
    prefix: &str,
    keep: usize,
) -> std::io::Result<()> {
    let mut snapshots: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(prefix) && n.ends_with(SNAPSHOT_SUFFIX))
        })
        .map(|e| e.path())
        .collect();
    // Sort by file name (which encodes the fixed-width timestamp), not by
    // full path. `read_dir` only returns entries from `dir`, so for now the
    // two are equivalent, but future refactors that pass nested or absolute
    // paths through here would silently sort by parent first and break the
    // chronological assumption. Sorting on `file_name` keeps the contract
    // invariant under that change.
    snapshots.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    let to_remove = snapshots.len().saturating_sub(keep);
    for old in snapshots.iter().take(to_remove) {
        if let Err(e) = std::fs::remove_file(old) {
            log::warn!("db: could not remove old snapshot {}: {e}", old.display());
        }
    }
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
    async fn post_groups_table_and_group_id_column_exist_after_migration_018() {
        // Migration 018 introduces sibling-row groups for cross-network
        // publishing. Two structural guarantees this test locks in:
        //   1. The `post_groups` parent table exists with the expected
        //      columns. The composer transaction depends on it.
        //   2. `post_history.group_id` exists and stays nullable. Legacy
        //      mono-network rows leave it NULL — backfill is intentionally
        //      skipped, so a NOT NULL constraint here would crash on
        //      first launch for any v0.3.8 user upgrading.
        // The cascade-on-delete behaviour is a soft contract enforced in
        // `db::groups::delete_keeping_children` (SQLite refuses
        // ALTER TABLE ADD COLUMN ... REFERENCES, see migration 013's
        // hotfix history). The test for that contract lives in
        // `db::groups::tests`.
        let pool = fresh_migrated_pool().await;

        // post_groups table exists.
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='post_groups'",
        )
        .fetch_one(&pool)
        .await
        .expect("query master");
        assert_eq!(count, 1, "migration 018 must create the post_groups table");

        let group_cols = table_columns(&pool, "post_groups").await;
        for c in ["id", "brief", "created_at"] {
            assert!(
                group_cols.contains(&c.to_string()),
                "post_groups missing column `{c}`, got: {group_cols:?}"
            );
        }

        // post_history.group_id exists.
        let history_cols = table_columns(&pool, "post_history").await;
        assert!(
            history_cols.contains(&"group_id".to_string()),
            "migration 018 must add post_history.group_id, got: {history_cols:?}"
        );

        // group_id must be nullable. PRAGMA table_info reports a
        // `notnull` flag; we check it stays at 0 so a backfill isn't
        // required for legacy single-network rows.
        let rows = sqlx::query("PRAGMA table_info(post_history)")
            .fetch_all(&pool)
            .await
            .expect("PRAGMA table_info");
        let group_id_notnull: i64 = rows
            .iter()
            .find(|r| r.get::<String, _>("name") == "group_id")
            .map(|r| r.get::<i64, _>("notnull"))
            .expect("group_id column missing");
        assert_eq!(
            group_id_notnull, 0,
            "post_history.group_id must be nullable for retrocompat with mono-network rows"
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

    /// Helper: list all rotation snapshots in `dir` for the given db filename.
    fn list_snapshots(dir: &std::path::Path, db_filename: &str) -> Vec<String> {
        let prefix = format!("{db_filename}.pre-migrate-");
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read tempdir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(&prefix) && n.ends_with(".bak"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn snapshot_creates_timestamped_file_and_drops_legacy() {
        // Upgrade case: app.db plus a leftover legacy unrotated
        // `app.db.pre-migrate.bak` from pre-rotation versions. The new
        // snapshot must land under the timestamped name and the legacy
        // file must be removed so it doesn't pollute the rotation count.
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("app.db");
        std::fs::write(&db_path, b"sqlite database bytes here").expect("write db");

        let legacy = tmp.path().join("app.db.pre-migrate.bak");
        std::fs::write(&legacy, b"old legacy snapshot").expect("write legacy");

        super::snapshot_db_before_migrate_with(&db_path, "20260510T120000000", 3)
            .expect("snapshot must succeed");

        let new_snap = tmp.path().join("app.db.pre-migrate-20260510T120000000.bak");
        assert!(new_snap.exists(), "timestamped snapshot must be created");
        assert_eq!(
            std::fs::read(&new_snap).expect("read snap"),
            b"sqlite database bytes here",
            "snapshot bytes must match the source",
        );
        assert!(
            !legacy.exists(),
            "legacy unrotated snapshot must be removed once a timestamped one exists",
        );
    }

    #[test]
    fn snapshot_is_noop_on_first_launch() {
        // Brand-new install: no app.db yet. Snapshot must succeed silently
        // and not create anything (otherwise we'd litter empty files in
        // the data dir on every fresh startup).
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("app.db");
        super::snapshot_db_before_migrate_with(&db_path, "20260510T120000000", 3)
            .expect("noop must succeed");
        assert!(list_snapshots(tmp.path(), "app.db").is_empty());
    }

    #[test]
    fn snapshot_keeps_at_most_three_newest_after_many_runs() {
        // Five sequential snapshots — only the three newest must survive.
        // The fixed-width timestamp gives us deterministic lexicographic
        // = chronological ordering across all five files.
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("app.db");
        std::fs::write(&db_path, b"db v1").expect("write db");

        let timestamps = [
            "20260510T120000001",
            "20260510T120000002",
            "20260510T120000003",
            "20260510T120000004",
            "20260510T120000005",
        ];
        for ts in timestamps {
            super::snapshot_db_before_migrate_with(&db_path, ts, 3).expect("snapshot must succeed");
        }

        let kept = list_snapshots(tmp.path(), "app.db");
        assert_eq!(
            kept.len(),
            3,
            "rotation must keep exactly 3 snapshots, got: {kept:?}",
        );
        // Verify the surviving set is the newest three (003, 004, 005).
        for ts in &timestamps[2..] {
            let name = format!("app.db.pre-migrate-{ts}.bak");
            assert!(
                kept.contains(&name),
                "must keep newest snapshot {name}, got: {kept:?}",
            );
        }
        // ...and the two oldest are gone.
        for ts in &timestamps[..2] {
            let name = format!("app.db.pre-migrate-{ts}.bak");
            assert!(
                !kept.contains(&name),
                "must have pruned older snapshot {name}, got: {kept:?}",
            );
        }
    }

    #[test]
    fn snapshot_naming_matches_legacy_with_extension_for_non_default_db() {
        // Regression guard: the previous implementation used
        // `db_path.with_extension("db.pre-migrate.bak")` which, for a DB
        // named `notes.sqlite3`, replaced `.sqlite3` and produced
        // `notes.db.pre-migrate.bak`. The rewrite must derive snapshot
        // names from the stem so non-default DB filenames still map onto
        // the legacy naming scheme — otherwise the legacy-cleanup branch
        // would silently miss the user's old snapshot file.
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("notes.sqlite3");
        std::fs::write(&db_path, b"db bytes").expect("write db");

        let legacy = tmp.path().join("notes.db.pre-migrate.bak");
        std::fs::write(&legacy, b"old legacy snapshot").expect("write legacy");

        super::snapshot_db_before_migrate_with(&db_path, "20260510T120000000", 3)
            .expect("snapshot must succeed");

        let new_snap = tmp
            .path()
            .join("notes.db.pre-migrate-20260510T120000000.bak");
        assert!(
            new_snap.exists(),
            "stem-derived timestamped snapshot must be created",
        );
        assert!(
            !legacy.exists(),
            "legacy `<stem>.db.pre-migrate.bak` must be removed even when DB is non-default",
        );
    }

    #[test]
    fn snapshot_rotation_does_not_touch_unrelated_bak_files() {
        // The data dir holds other caches (e.g. test fixtures, user-named
        // backups). Pruning must only target files matching our prefix —
        // never an arbitrary `.bak` next door.
        let tmp = tempfile::tempdir().expect("create tempdir");
        let db_path = tmp.path().join("app.db");
        std::fs::write(&db_path, b"db").expect("write db");

        let unrelated_a = tmp.path().join("app.db.before-recovery.bak");
        let unrelated_b = tmp.path().join("notes.bak");
        std::fs::write(&unrelated_a, b"manual recovery copy").expect("write unrelated a");
        std::fs::write(&unrelated_b, b"unrelated bak").expect("write unrelated b");

        // Run enough rotations to trigger the pruner.
        for ts in [
            "20260510T120000001",
            "20260510T120000002",
            "20260510T120000003",
            "20260510T120000004",
        ] {
            super::snapshot_db_before_migrate_with(&db_path, ts, 3).expect("snapshot");
        }

        assert!(
            unrelated_a.exists(),
            "manual recovery copy must be untouched"
        );
        assert!(unrelated_b.exists(), "unrelated .bak must be untouched");
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
