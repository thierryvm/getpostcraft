/// Auto-backup + first-launch restore prompt.
///
/// ## Why this exists
///
/// 2026-05-08: the project owner uninstalled v0.2.0 to fix a migration
/// mismatch and reinstalled v0.3.1. The Tauri NSIS uninstaller's
/// "Remove user data" prompt was checked by default and wiped the
/// SQLite DB plus every keyring entry under `app.getpostcraft.*`.
/// All posts, ProductTruth, OAuth connections, and AI keys gone.
///
/// The fix has three layers, this module ships layers 1 and 2:
///
///   1. **Daily auto-export** to `~/Documents/Getpostcraft/backups/`.
///      The Documents folder lives outside `AppData` and is NOT touched
///      by the uninstaller, so the backup survives a wipe.
///   2. **First-launch restore prompt** when the DB is fresh AND a
///      backup is available. The user gets a one-click recovery path
///      instead of the silent data loss this PR was born from.
///   3. **NSIS uncheck-by-default** for the wipe option — handled in
///      tauri.conf.json bundle settings, separate follow-up.
use crate::state::AppState;
use serde::Serialize;
use std::path::PathBuf;

/// Where auto-backups land. Documents folder is the right pick because:
///   - It survives uninstall (unlike `AppData\Roaming\getpostcraft`).
///   - It's user-visible — the user can see the backups exist and
///     understand the recovery path without digging through hidden dirs.
///   - It's per-user, not system-wide — no permission issues.
pub fn auto_backup_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Getpostcraft")
        .join("backups")
}

/// Number of daily backups to keep. Beyond this we rotate the oldest
/// out — 7 days of history strikes the balance between disk use
/// (each backup is ~1-50 MB depending on post count) and recovery
/// granularity (a week of misclicks is recoverable).
const KEEP_DAILY: usize = 7;

/// Minimum age of the latest backup before we create a new one. 23 hours
/// (not 24) so a user who launches the app at the same time daily still
/// gets a fresh backup.
const MIN_AGE_SECS: u64 = 23 * 3600;

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub path: String,
    /// ISO 8601 UTC of the file modification time.
    pub exported_at: String,
    /// Days elapsed between backup and now — convenient for the UI label.
    pub age_days: i64,
    pub size_bytes: u64,
}

/// List all `.gpcbak` files in the auto-backup dir, newest first.
pub fn list_backups() -> Vec<BackupInfo> {
    let dir = auto_backup_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("gpcbak") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };

        let dt: chrono::DateTime<chrono::Utc> = modified.into();
        let age_days = (chrono::Utc::now() - dt).num_days();

        out.push(BackupInfo {
            path: path.to_string_lossy().to_string(),
            exported_at: dt.to_rfc3339(),
            age_days,
            size_bytes: meta.len(),
        });
    }
    // Newest first.
    out.sort_by(|a, b| b.exported_at.cmp(&a.exported_at));
    out
}

/// Return the most recent backup if any.
pub fn latest_backup() -> Option<BackupInfo> {
    list_backups().into_iter().next()
}

/// Trigger an auto-backup if the latest one is older than `MIN_AGE_SECS`
/// or none exists. Reuses the same `VACUUM INTO` + ZIP logic as the
/// user-facing `.gpcbak` export — see `data_export::export_backup_zip`.
/// Output filename pattern: `getpostcraft-auto-{YYYYMMDD_HHMMSS}.gpcbak`.
///
/// Returns the new backup path on creation, or None if a recent one
/// already existed (no work needed).
pub async fn run_if_due(state: &AppState) -> Result<Option<String>, String> {
    let dir = auto_backup_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Create backup dir: {e}"))?;

    if let Some(latest) = latest_backup() {
        let dt = chrono::DateTime::parse_from_rfc3339(&latest.exported_at)
            .map_err(|e| format!("Parse backup ts: {e}"))?;
        let age = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_seconds();
        if age >= 0 && (age as u64) < MIN_AGE_SECS {
            // Latest is fresh enough — nothing to do.
            return Ok(None);
        }
    }

    // Snapshot the live DB the same way the user-facing export does, but
    // write to the Documents folder instead of Downloads.
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_path = dir.join(format!("getpostcraft-auto-{timestamp}.gpcbak"));
    crate::commands::data_export::write_backup_to(&state.db, &output_path).await?;

    // Rotate.
    prune_old_backups(KEEP_DAILY);

    Ok(Some(output_path.to_string_lossy().to_string()))
}

/// Drop backups beyond the retention count. Newest first, drop from the tail.
fn prune_old_backups(keep: usize) {
    let backups = list_backups();
    for stale in backups.into_iter().skip(keep) {
        let _ = std::fs::remove_file(&stale.path);
    }
}

/// True when the DB has no user data — fresh install OR post-wipe.
/// Used by the first-launch restore prompt to decide whether to surface.
pub async fn is_db_fresh(pool: &sqlx::SqlitePool) -> bool {
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if accounts > 0 {
        return false;
    }
    let posts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM post_history")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    posts == 0
}

// ── Tauri commands ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RestoreOffer {
    pub backup: BackupInfo,
}

/// Returns a restore offer if the DB is fresh AND a backup is on disk.
/// The renderer mounts a one-shot dialog on startup and queries this.
#[tauri::command]
pub async fn get_restore_offer(
    state: tauri::State<'_, AppState>,
) -> Result<Option<RestoreOffer>, String> {
    if !is_db_fresh(&state.db).await {
        return Ok(None);
    }
    Ok(latest_backup().map(|backup| RestoreOffer { backup }))
}

/// Lists every `.gpcbak` in the backup dir for a manual-restore picker
/// (Settings → Données could surface this — out of scope for V1 but the
/// API is here when we want it).
#[tauri::command]
pub fn list_auto_backups() -> Vec<BackupInfo> {
    list_backups()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_backup_dir_under_user_documents() {
        let p = auto_backup_dir();
        assert!(
            p.ends_with("Getpostcraft/backups") || p.ends_with("Getpostcraft\\backups"),
            "path must end with the Getpostcraft/backups suffix, got {p:?}"
        );
    }

    #[test]
    fn keep_daily_rotation_is_seven() {
        // Regression guard: documenting the retention default. Bumping this
        // affects disk usage on every user — make it deliberate.
        assert_eq!(KEEP_DAILY, 7);
    }

    #[test]
    fn min_age_is_close_to_one_day() {
        // Same intent — we want roughly daily, slightly under so the user
        // launching at 9 a.m. every day still gets a fresh one. Hard
        // assertion to catch accidental swaps to seconds vs hours.
        assert_eq!(MIN_AGE_SECS, 23 * 3600);
        assert!(MIN_AGE_SECS < 24 * 3600);
    }

    #[tokio::test]
    async fn is_db_fresh_returns_true_on_empty_db() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        assert!(is_db_fresh(&pool).await, "fresh DB after migrations is empty");
    }

    #[tokio::test]
    async fn is_db_fresh_returns_false_with_an_account_row() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO accounts (provider, user_id, username, display_name, token_key) \
             VALUES ('instagram', 'u1', 'name', NULL, 'instagram:u1')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(!is_db_fresh(&pool).await, "account row must mark DB non-fresh");
    }

    #[tokio::test]
    async fn is_db_fresh_returns_false_with_a_post_row() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO post_history (network, caption, hashtags, status, created_at) \
             VALUES ('instagram', 'caption', '[]', 'draft', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(!is_db_fresh(&pool).await, "post row must mark DB non-fresh");
    }
}
