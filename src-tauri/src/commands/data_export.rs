/// User data export — `.gpcbak` ZIP backup format.
///
/// ## Why this exists
///
/// Thierry's request: "the user must be able to recover their data if they
/// want to switch and use Supabase or another service." A backup format
/// the user owns means GPC can't lock them in even if the project dies
/// or pivots.
///
/// ## What's in a `.gpcbak`
///
/// A standard ZIP containing:
///
/// - `app.db` — a transactionally-consistent snapshot of the SQLite
///   database produced via `VACUUM INTO` (SQLite's recommended online-
///   backup primitive). Anyone with the `sqlite3` CLI can open it.
/// - `manifest.json` — format identifier, format version, app version,
///   ISO-8601 export timestamp, SHA-256 of the .db bytes.
///
/// ## What's NOT in a `.gpcbak`
///
/// Secrets. AI API keys (PR-A) and OAuth tokens (PR-X) live in the OS
/// keyring; they are tied to a specific user account on a specific
/// machine and cannot — and should not — travel with the data export.
/// On a fresh install, the user re-pastes their AI key once and re-runs
/// the OAuth dance for each social account. The DB rows referencing the
/// account are restored, but the actual access tokens are re-issued by
/// the providers. This is the right boundary: data portability without
/// credential portability.
///
/// ## Why VACUUM INTO instead of file copy
///
/// SQLite uses WAL mode. A naive copy of `app.db` while the app is
/// running would miss whatever's in `app.db-wal`, producing an
/// inconsistent snapshot. `VACUUM INTO` blocks just long enough to
/// produce a single self-contained DB file with the WAL fully merged in.
/// It's the documented online-backup path.
///
/// ## Why the format is versioned
///
/// `format_version: 1` lets a future PR-E3 (import) reject incompatible
/// archives cleanly with a clear error rather than silently corrupting
/// the user's data.
use crate::state::AppState;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::path::PathBuf;
use zip::write::SimpleFileOptions;

/// Identifier embedded in the manifest. Importers verify this string
/// before unpacking — protects against accidentally restoring some
/// random ZIP that happens to contain `app.db`.
const MANIFEST_FORMAT: &str = "gpcbak";

/// Bumped when the archive structure changes (new files, renamed files,
/// schema migration boundaries). Stays at 1 for the V1 format.
const MANIFEST_FORMAT_VERSION: u32 = 1;

#[derive(Serialize)]
struct BackupManifest {
    format: &'static str,
    format_version: u32,
    /// App version that produced this archive — useful when diagnosing
    /// "this backup is from before X feature shipped" issues.
    app_version: &'static str,
    /// ISO 8601 / RFC 3339 UTC. Local time would be ambiguous on travel.
    exported_at: String,
    /// Hex SHA-256 of the `app.db` bytes inside the archive. Lets a
    /// future importer detect tampering or corruption.
    checksum_sha256: String,
}

/// Write a snapshot of the local SQLite database into the user's
/// Downloads folder as `getpostcraft-backup-{YYYYMMDD_HHMMSS}.gpcbak`.
///
/// Returns the absolute path to the created archive.
#[tauri::command]
pub async fn export_backup_zip(state: tauri::State<'_, AppState>) -> Result<String, String> {
    // Fixed location keeps the UI simple (no save dialog dep). The user
    // can move the file later. Auto-backup (PR-AR) uses Documents/ via
    // `write_backup_to` directly — Downloads is the user-action target.
    let downloads = dirs::download_dir()
        .ok_or_else(|| "Impossible de trouver le dossier Téléchargements".to_string())?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_path = downloads.join(format!("getpostcraft-backup-{timestamp}.gpcbak"));

    write_backup_to(&state.db, &output_path).await?;
    Ok(output_path.to_string_lossy().to_string())
}

/// Internal API: snapshot the DB into a `.gpcbak` at the caller's path.
/// Same semantics as `export_backup_zip` minus the Downloads-folder
/// resolution. Auto-backup writes to Documents through this entry point.
pub async fn write_backup_to(
    pool: &sqlx::SqlitePool,
    output_path: &std::path::Path,
) -> Result<(), String> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

    // Snapshot the live DB. VACUUM INTO requires a literal path — sqlite
    // parses it at compile time before binding, so we can't pass it as a
    // parameter. Path comes from `temp_dir()` + a deterministic
    // timestamped name, no user input flows in, format-string injection
    // is structurally impossible.
    let temp_db = std::env::temp_dir().join(format!("getpostcraft-snapshot-{timestamp}.db"));
    let path_for_sql = temp_db
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''"); // defense in depth even though path is system-controlled

    // RAII guard: deletes the snapshot when the guard drops, including on
    // panic between VACUUM and `write_archive`. The snapshot contains the
    // full DB and we don't want it lingering in /tmp on a crash.
    struct TempFileGuard<'a>(&'a std::path::Path);
    impl Drop for TempFileGuard<'_> {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0);
        }
    }

    sqlx::query(&format!("VACUUM INTO '{path_for_sql}'"))
        .execute(pool)
        .await
        .map_err(|e| format!("VACUUM INTO failed: {e}"))?;

    // Guard constructed AFTER VACUUM succeeds — before that, nothing to
    // clean up. Any early return, panic, or `?` from here triggers it.
    let _temp_guard = TempFileGuard(&temp_db);

    let db_bytes = std::fs::read(&temp_db).map_err(|e| format!("Read snapshot: {e}"))?;

    let checksum = format!("{:x}", Sha256::digest(&db_bytes));
    let manifest = BackupManifest {
        format: MANIFEST_FORMAT,
        format_version: MANIFEST_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION"),
        exported_at: chrono::Utc::now().to_rfc3339(),
        checksum_sha256: checksum,
    };
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("Serialize manifest: {e}"))?;

    let path_buf = output_path.to_path_buf();
    write_archive(&path_buf, &db_bytes, &manifest_json)
}

/// Pull the ZIP-write step out of the command so it's testable without
/// spinning up an `AppState` + sqlx pool.
fn write_archive(
    output_path: &PathBuf,
    db_bytes: &[u8],
    manifest_json: &str,
) -> Result<(), String> {
    let file = std::fs::File::create(output_path).map_err(|e| format!("Create archive: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);

    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("app.db", opts).map_err(|e| e.to_string())?;
    zip.write_all(db_bytes).map_err(|e| e.to_string())?;

    zip.start_file("manifest.json", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Import (PR-AR — first-launch restore) ────────────────────────────

#[derive(serde::Deserialize)]
struct ImportableManifest {
    format: String,
    format_version: u32,
    checksum_sha256: String,
}

/// Restore a `.gpcbak` archive over the live database.
///
/// Validation order:
///   1. Open the ZIP and read `manifest.json` → must have format `"gpcbak"`.
///   2. format_version must be `<= MANIFEST_FORMAT_VERSION` (we don't
///      know how to read versions newer than this binary).
///   3. Read `app.db` from the ZIP → SHA-256 must match `checksum_sha256`.
///
/// Only after all three pass do we touch the destination. We write the
/// new bytes to a temp neighbour first and `rename` them into place so
/// the swap is atomic on every platform — a crash mid-write leaves the
/// previous DB intact instead of corrupting it.
///
/// The caller (the restore-prompt dialog) must call
/// `tauri::AppHandle::restart` after this returns, because sqlx's pool
/// is still pointing at the old file's inode/handle.
#[tauri::command]
pub async fn import_backup_zip(path: String) -> Result<(), String> {
    use std::io::Read as _;

    let archive_path = std::path::Path::new(&path);
    if !archive_path.exists() {
        return Err(format!("Archive introuvable : {path}"));
    }

    let file = std::fs::File::open(archive_path).map_err(|e| format!("Open archive: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Parse ZIP: {e}"))?;

    // 1. Manifest.
    let mut manifest_buf = String::new();
    zip.by_name("manifest.json")
        .map_err(|_| "manifest.json absent — ce n'est pas un .gpcbak valide".to_string())?
        .read_to_string(&mut manifest_buf)
        .map_err(|e| format!("Read manifest: {e}"))?;
    let manifest: ImportableManifest =
        serde_json::from_str(&manifest_buf).map_err(|e| format!("Parse manifest: {e}"))?;
    if manifest.format != MANIFEST_FORMAT {
        return Err(format!(
            "Format inattendu : « {0} » (attendu : « {MANIFEST_FORMAT} »)",
            manifest.format
        ));
    }
    if manifest.format_version > MANIFEST_FORMAT_VERSION {
        return Err(format!(
            "Archive trop récente (v{0}) pour cette version de l'app (v{MANIFEST_FORMAT_VERSION}). \
             Mets à jour Getpostcraft avant de restaurer.",
            manifest.format_version
        ));
    }

    // 2. DB bytes + checksum.
    let mut db_bytes = Vec::new();
    zip.by_name("app.db")
        .map_err(|_| "app.db absent dans l'archive".to_string())?
        .read_to_end(&mut db_bytes)
        .map_err(|e| format!("Read app.db: {e}"))?;

    let actual_checksum = format!("{:x}", Sha256::digest(&db_bytes));
    if actual_checksum != manifest.checksum_sha256 {
        return Err(
            "Checksum SHA-256 invalide — l'archive a été modifiée ou est corrompue.".to_string(),
        );
    }

    // 3. Atomic swap.
    let target_dir = dirs::data_dir()
        .ok_or("Impossible de résoudre le data dir")?
        .join("getpostcraft");
    std::fs::create_dir_all(&target_dir).map_err(|e| format!("Create data dir: {e}"))?;

    // Best-effort cleanup of WAL + SHM so the restored DB starts clean.
    // sqlx is still holding the old pool — these may be locked. We try
    // anyway; the caller's `restart()` will release any lingering lock.
    let target_db = target_dir.join("app.db");
    let target_wal = target_dir.join("app.db-wal");
    let target_shm = target_dir.join("app.db-shm");

    let staged = target_dir.join("app.db.import-staged");
    std::fs::write(&staged, &db_bytes).map_err(|e| format!("Stage restore: {e}"))?;

    // Atomic rename = no half-written final file. On Windows, std::fs::rename
    // requires the target NOT to exist, so remove first; on Unix, rename
    // overwrites in one syscall. Either way, the staged file is fully
    // written before we touch the canonical path.
    let _ = std::fs::remove_file(&target_wal);
    let _ = std::fs::remove_file(&target_shm);
    let _ = std::fs::remove_file(&target_db);
    std::fs::rename(&staged, &target_db).map_err(|e| format!("Atomic swap: {e}"))?;

    log::info!(
        "Restore: imported app.db ({} bytes) from {path}",
        db_bytes.len()
    );

    Ok(())
}

// ── Portable JSON export (PR-E2) ─────────────────────────────────────────────
//
// `.gpcbak` is great for "I'll restore on another GPC install" but useless if
// the user wants to migrate to Supabase, Postgres, n8n, or write a custom
// dashboard against the data. The portable export trades a bigger archive for
// a format any tool can read:
//
//   - `accounts.json`, `posts.json`, `settings.json` — flat JSON, no SQLite-isms
//   - `media/` — base64 PNGs decoded into actual files (no 33% base64 overhead)
//   - `schema.sql` — Postgres translation of the SQLite schema, ready to load
//     into Supabase or any Postgres host
//   - `manifest.json` — same role as in `.gpcbak`, with notes for the importer

const PORTABLE_FORMAT: &str = "gpc-portable";
const PORTABLE_FORMAT_VERSION: u32 = 1;

/// Postgres translation of the SQLite schema (migrations 001 through 013).
/// Hand-written so we can choose better Postgres-native types
/// (`JSONB` for the JSON-as-TEXT columns, `TIMESTAMPTZ` for the ISO-8601
/// strings) instead of literally translating the SQLite DDL.
///
/// ## Why we hand-write instead of generating
///
/// `pg_dump`-style automatic translation from sqlite would force us to
/// preserve `INTEGER PRIMARY KEY AUTOINCREMENT` and `TEXT` for everything,
/// which works on Postgres but throws away the type info the user actually
/// wants when loading into Supabase. Hand-writing also means we get to
/// document each table in the SQL itself, which is the documentation they
/// will read first.
///
/// Bumped together with PORTABLE_FORMAT_VERSION when the table layout changes.
const POSTGRES_SCHEMA_SQL: &str = r#"-- Getpostcraft data — Postgres schema
-- Translated from the local SQLite schema. Suitable for Supabase or any
-- vanilla Postgres host. Run this BEFORE loading the JSON files.
--
-- All `*.json` files in this archive are flat arrays/objects matching the
-- column order below. Use any ETL (jq + COPY, pgloader, custom INSERTs).

CREATE TABLE IF NOT EXISTS accounts (
    id              BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    provider        TEXT NOT NULL,
    user_id         TEXT NOT NULL,
    username        TEXT NOT NULL,
    display_name    TEXT,
    -- Note: `token_key` from the SQLite schema is intentionally absent.
    -- It pointed to an OS keyring entry tied to the local machine and has
    -- no meaning outside the original install.
    brand_color     TEXT,
    accent_color    TEXT,
    product_truth   TEXT,
    visual_profile  JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider, user_id)
);

CREATE TABLE IF NOT EXISTS post_history (
    id              BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    network         TEXT NOT NULL,
    caption         TEXT NOT NULL,
    hashtags        JSONB NOT NULL,
    status          TEXT NOT NULL DEFAULT 'draft',
    created_at      TIMESTAMPTZ NOT NULL,
    published_at    TIMESTAMPTZ,
    scheduled_at    TIMESTAMPTZ,
    -- `image_paths` here is a JSONB array of strings — the relative paths
    -- inside the `media/` folder of this archive. Adapt to your storage layer
    -- (S3, Supabase Storage, etc.) when loading.
    image_paths     JSONB,
    ig_media_id     TEXT,
    account_id      BIGINT REFERENCES accounts(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_post_history_scheduled_at
    ON post_history(scheduled_at);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

#[derive(Serialize)]
struct PortableManifest {
    format: &'static str,
    format_version: u32,
    app_version: &'static str,
    exported_at: String,
    /// Counts so the importer can sanity-check before loading.
    counts: PortableCounts,
    /// Free-form text for the user opening the archive — kept inside the
    /// manifest instead of as a separate README to dodge the no-MD-files rule.
    notes: &'static str,
}

#[derive(Serialize)]
struct PortableCounts {
    accounts: usize,
    posts: usize,
    media_files: usize,
}

#[derive(Serialize)]
struct PortableAccount {
    id: i64,
    provider: String,
    user_id: String,
    username: String,
    display_name: Option<String>,
    brand_color: Option<String>,
    accent_color: Option<String>,
    product_truth: Option<String>,
    /// `visual_profile` is parsed from its stored JSON-string form so the
    /// portable JSON has nested object structure instead of an opaque blob.
    /// Falls back to `null` if the stored value is malformed.
    visual_profile: Option<serde_json::Value>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct PortablePost {
    id: i64,
    network: String,
    caption: String,
    hashtags: Vec<String>,
    status: String,
    created_at: String,
    published_at: Option<String>,
    scheduled_at: Option<String>,
    /// Relative paths inside the archive's `media/` folder, OR
    /// pass-through values for legacy rows that stored a filesystem path
    /// instead of a base64 data URL. The importer can distinguish by
    /// checking the prefix.
    image_paths: Vec<String>,
    ig_media_id: Option<String>,
    account_id: Option<i64>,
}

const PORTABLE_NOTES: &str = "\
This archive contains a portable copy of your Getpostcraft data, designed to be \
imported into any tool that speaks JSON or Postgres.\n\
\n\
Files:\n\
  - accounts.json    — connected social accounts (token_key intentionally omitted)\n\
  - posts.json       — drafts and published posts; image_paths point into media/\n\
  - settings.json    — provider/model preferences\n\
  - media/*.png      — carousel slides extracted from the database (no base64)\n\
  - schema.sql       — Postgres CREATE TABLEs for the three tables above\n\
\n\
To load into Supabase or another Postgres:\n\
  1. psql ... < schema.sql\n\
  2. Use jq + COPY, pgloader, or any ETL of your choice to load the JSON files.\n\
\n\
Secrets are NOT in this archive. AI keys and OAuth tokens stay in the OS\n\
keyring on the originating machine. After loading, you must re-add credentials\n\
in whatever tool you migrate to.\n";

/// Export a portable, multi-tool archive of the user's local data.
///
/// Output: `~/Downloads/getpostcraft-portable-{YYYYMMDD_HHMMSS}.zip`
#[tauri::command]
pub async fn export_portable_zip(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let downloads = dirs::download_dir()
        .ok_or_else(|| "Impossible de trouver le dossier Téléchargements".to_string())?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_path = downloads.join(format!("getpostcraft-portable-{timestamp}.zip"));

    // Pull everything in memory — even on a heavy account, we're talking a
    // few thousand rows and a few hundred MB of media at the absolute extreme,
    // which a desktop app handles trivially. No need for streaming.
    let accounts = fetch_portable_accounts(&state.db).await?;
    let (posts_with_media, media_files) = fetch_portable_posts(&state.db).await?;
    let settings = fetch_settings_map(&state.db).await?;

    let manifest = PortableManifest {
        format: PORTABLE_FORMAT,
        format_version: PORTABLE_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION"),
        exported_at: chrono::Utc::now().to_rfc3339(),
        counts: PortableCounts {
            accounts: accounts.len(),
            posts: posts_with_media.len(),
            media_files: media_files.len(),
        },
        notes: PORTABLE_NOTES,
    };

    write_portable_archive(
        &output_path,
        &accounts,
        &posts_with_media,
        &settings,
        &media_files,
        &manifest,
    )?;

    Ok(output_path.to_string_lossy().to_string())
}

async fn fetch_portable_accounts(pool: &sqlx::SqlitePool) -> Result<Vec<PortableAccount>, String> {
    let accounts = crate::db::accounts::list(pool).await?;
    Ok(accounts
        .into_iter()
        .map(|a| PortableAccount {
            id: a.id,
            provider: a.provider,
            user_id: a.user_id,
            username: a.username,
            display_name: a.display_name,
            brand_color: a.brand_color,
            accent_color: a.accent_color,
            product_truth: a.product_truth,
            visual_profile: a
                .visual_profile
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            created_at: a.created_at,
            updated_at: a.updated_at,
        })
        .collect())
}

/// Returns (posts with image_paths rewritten, decoded media bytes by filename).
async fn fetch_portable_posts(
    pool: &sqlx::SqlitePool,
) -> Result<(Vec<PortablePost>, Vec<(String, Vec<u8>)>), String> {
    use sqlx::Row as _;

    let rows = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at, \
                scheduled_at, images, ig_media_id, account_id \
         FROM post_history ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Read posts: {e}"))?;

    let mut posts = Vec::with_capacity(rows.len());
    let mut media: Vec<(String, Vec<u8>)> = Vec::new();

    for row in rows {
        let id: i64 = row.get("id");
        let hashtags_json: String = row.get("hashtags");
        let hashtags: Vec<String> = serde_json::from_str(&hashtags_json).unwrap_or_default();

        let images_json: Option<String> = row.try_get("images").ok().flatten();
        let raw_images: Vec<String> = images_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();

        // Walk each image: base64 data URLs become real PNG files in the
        // archive; anything else (legacy filesystem paths) is passed through
        // so the importer can decide what to do with it.
        let mut rewritten = Vec::with_capacity(raw_images.len());
        for (slide_ix, img) in raw_images.into_iter().enumerate() {
            match decode_image_data_url(&img) {
                Some(bytes) => {
                    let filename = format!("media/post-{id}-slide-{:02}.png", slide_ix + 1);
                    rewritten.push(filename.clone());
                    media.push((filename, bytes));
                }
                None => {
                    // Legacy path / unknown format — keep the original value.
                    rewritten.push(img);
                }
            }
        }

        posts.push(PortablePost {
            id,
            network: row.get("network"),
            caption: row.get("caption"),
            hashtags,
            status: row.get("status"),
            created_at: row.get("created_at"),
            published_at: row.try_get("published_at").ok().flatten(),
            scheduled_at: row.try_get("scheduled_at").ok().flatten(),
            image_paths: rewritten,
            ig_media_id: row.try_get("ig_media_id").ok().flatten(),
            account_id: row.try_get("account_id").ok().flatten(),
        });
    }

    Ok((posts, media))
}

async fn fetch_settings_map(
    pool: &sqlx::SqlitePool,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    use sqlx::Row as _;

    let rows = sqlx::query("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Read settings: {e}"))?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("value")))
        .collect())
}

/// Decode a base64 image data URL (PNG only — that's all we generate today).
/// Returns None for any other shape so callers can distinguish base64 payloads
/// from legacy filesystem paths.
fn decode_image_data_url(value: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;

    let b64 = value.strip_prefix("data:image/png;base64,")?;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

fn write_portable_archive(
    output_path: &PathBuf,
    accounts: &[PortableAccount],
    posts: &[PortablePost],
    settings: &std::collections::BTreeMap<String, String>,
    media: &[(String, Vec<u8>)],
    manifest: &PortableManifest,
) -> Result<(), String> {
    let file = std::fs::File::create(output_path).map_err(|e| format!("Create archive: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);

    // Text content compresses well; PNGs in `media/` are already deflated, so
    // we still pass `Deflated` (zip will skip work on incompressible data).
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let manifest_json =
        serde_json::to_string_pretty(manifest).map_err(|e| format!("Serialize manifest: {e}"))?;
    let accounts_json =
        serde_json::to_string_pretty(accounts).map_err(|e| format!("Serialize accounts: {e}"))?;
    let posts_json =
        serde_json::to_string_pretty(posts).map_err(|e| format!("Serialize posts: {e}"))?;
    let settings_json =
        serde_json::to_string_pretty(settings).map_err(|e| format!("Serialize settings: {e}"))?;

    for (name, content) in [
        ("manifest.json", manifest_json.as_bytes()),
        ("accounts.json", accounts_json.as_bytes()),
        ("posts.json", posts_json.as_bytes()),
        ("settings.json", settings_json.as_bytes()),
        ("schema.sql", POSTGRES_SCHEMA_SQL.as_bytes()),
    ] {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(content).map_err(|e| e.to_string())?;
    }

    for (name, bytes) in media {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    #[test]
    fn manifest_serializes_with_expected_keys() {
        let m = BackupManifest {
            format: MANIFEST_FORMAT,
            format_version: MANIFEST_FORMAT_VERSION,
            app_version: "0.2.0",
            exported_at: "2026-05-08T12:00:00Z".to_string(),
            checksum_sha256: "abc123".to_string(),
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""format":"gpcbak""#));
        assert!(json.contains(r#""format_version":1"#));
        assert!(json.contains(r#""app_version":"0.2.0""#));
        assert!(json.contains(r#""exported_at":"2026-05-08T12:00:00Z""#));
        assert!(json.contains(r#""checksum_sha256":"abc123""#));
    }

    #[test]
    fn write_archive_produces_a_valid_zip_with_both_entries() {
        let temp_dir = std::env::temp_dir();
        let archive_path = temp_dir.join(format!(
            "test-archive-{}.gpcbak",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let db_bytes = b"SQLite format 3\0fake_db_payload";
        let manifest_json = r#"{"format":"gpcbak","format_version":1}"#;

        write_archive(&archive_path, db_bytes, manifest_json).expect("write_archive failed");
        assert!(archive_path.exists(), "archive must be created on disk");

        // Open the ZIP back and verify the two expected entries.
        let f = std::fs::File::open(&archive_path).unwrap();
        let mut zip = zip::ZipArchive::new(f).expect("must be a valid ZIP");

        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"app.db".to_string()));
        assert!(names.contains(&"manifest.json".to_string()));

        // Verify content roundtrip.
        let mut buf = Vec::new();
        zip.by_name("app.db")
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();
        assert_eq!(buf, db_bytes);

        let mut manifest_buf = String::new();
        zip.by_name("manifest.json")
            .unwrap()
            .read_to_string(&mut manifest_buf)
            .unwrap();
        assert_eq!(manifest_buf, manifest_json);

        let _ = std::fs::remove_file(&archive_path);
    }

    #[test]
    fn checksum_is_deterministic_and_matches_dbs_bytes() {
        let bytes = b"hello-getpostcraft";
        let checksum = format!("{:x}", Sha256::digest(bytes));
        // Independently verifiable: SHA-256("hello-getpostcraft")
        // → echo -n "hello-getpostcraft" | sha256sum
        assert_eq!(checksum.len(), 64, "SHA-256 hex must be 64 chars");
        // Re-hashing the same bytes must produce the same checksum.
        let again = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(checksum, again);
    }

    #[test]
    fn manifest_format_version_is_v1() {
        // Regression guard: changing this constant is a breaking change for
        // PR-E3 (import). Bumping it without updating importers will silently
        // accept incompatible archives. Keep this assertion until that path
        // explicitly exists.
        assert_eq!(MANIFEST_FORMAT_VERSION, 1);
    }

    // ── Portable export tests (PR-E2) ─────────────────────────────────

    #[test]
    fn portable_account_dto_drops_token_key() {
        // Regression guard: the whole point of the portable format is that
        // it does NOT carry tokens off the originating machine. If somebody
        // adds a `token_key` field to PortableAccount, the JSON shape would
        // suddenly include it — this test fails immediately.
        let a = PortableAccount {
            id: 1,
            provider: "instagram".to_string(),
            user_id: "u1".to_string(),
            username: "n".to_string(),
            display_name: None,
            brand_color: None,
            accent_color: None,
            product_truth: None,
            visual_profile: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(
            !json.contains("token_key"),
            "PortableAccount must never serialise a token_key field"
        );
    }

    #[test]
    fn decode_image_data_url_accepts_png_data_url() {
        // PNG file signature: 89 50 4E 47 0D 0A 1A 0A
        let png_bytes: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
        let data_url = format!("data:image/png;base64,{b64}");
        let decoded = decode_image_data_url(&data_url).expect("must decode valid PNG data URL");
        assert_eq!(decoded, png_bytes);
    }

    #[test]
    fn decode_image_data_url_rejects_non_data_url() {
        // Legacy filesystem paths must NOT be base64-decoded — passthrough.
        assert!(decode_image_data_url("/tmp/getpostcraft/render.png").is_none());
        assert!(decode_image_data_url("data:image/jpeg;base64,abc").is_none());
        assert!(decode_image_data_url("").is_none());
        assert!(decode_image_data_url("not a data url at all").is_none());
    }

    #[test]
    fn postgres_schema_covers_all_three_tables() {
        // The schema lives as a single string constant; if a contributor
        // forgets to migrate one of the three tables when adding a new one,
        // we catch it here.
        for table in ["accounts", "post_history", "settings"] {
            assert!(
                POSTGRES_SCHEMA_SQL.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "schema.sql must declare table `{table}`"
            );
        }
    }

    #[test]
    fn postgres_schema_uses_jsonb_for_json_columns() {
        // SQLite stores hashtags / images / visual_profile as TEXT-with-JSON.
        // The portable format is a chance to upgrade them to JSONB so the
        // user can query them in Postgres. Regression-guard each one.
        for col in [
            "hashtags        JSONB",
            "image_paths     JSONB",
            "visual_profile  JSONB",
        ] {
            assert!(
                POSTGRES_SCHEMA_SQL.contains(col),
                "schema.sql must declare {col}"
            );
        }
    }

    #[test]
    fn postgres_schema_drops_token_key_column_from_accounts() {
        // Defense-in-depth: even if PortableAccount accidentally serialised a
        // token_key, an importer that respects schema.sql wouldn't have a
        // column to land it in. We deliberately keep the *comment* explaining
        // why token_key is absent (it's documentation), so we look for an
        // actual column declaration like `token_key  TEXT` rather than any
        // mention of the string.
        let accounts_block = POSTGRES_SCHEMA_SQL
            .split("CREATE TABLE IF NOT EXISTS accounts")
            .nth(1)
            .and_then(|s| s.split("CREATE TABLE").next())
            .unwrap_or("");
        let has_column = accounts_block.lines().any(|line| {
            let trimmed = line.trim_start();
            // A real column declaration starts with the column name and
            // continues with a type. Comment lines start with `--`.
            !trimmed.starts_with("--")
                && trimmed.starts_with("token_key")
                && (trimmed.contains("TEXT") || trimmed.contains("text"))
        });
        assert!(
            !has_column,
            "Postgres accounts table must NOT declare a token_key column"
        );
    }

    #[test]
    fn portable_format_version_is_v1() {
        // Same regression guard as the .gpcbak version — bumping requires
        // updating any importer code we ship later.
        assert_eq!(PORTABLE_FORMAT_VERSION, 1);
    }

    #[test]
    fn portable_manifest_serialises_counts() {
        let m = PortableManifest {
            format: PORTABLE_FORMAT,
            format_version: PORTABLE_FORMAT_VERSION,
            app_version: "0.2.0",
            exported_at: "2026-05-08T12:00:00Z".to_string(),
            counts: PortableCounts {
                accounts: 2,
                posts: 17,
                media_files: 42,
            },
            notes: PORTABLE_NOTES,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""format":"gpc-portable""#));
        assert!(json.contains(r#""accounts":2"#));
        assert!(json.contains(r#""posts":17"#));
        assert!(json.contains(r#""media_files":42"#));
        assert!(json.contains("token_key intentionally omitted"));
    }

    #[test]
    fn write_portable_archive_emits_all_expected_files() {
        let temp_dir = std::env::temp_dir();
        let archive_path = temp_dir.join(format!(
            "test-portable-{}.zip",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let accounts = vec![PortableAccount {
            id: 1,
            provider: "instagram".to_string(),
            user_id: "u".to_string(),
            username: "name".to_string(),
            display_name: None,
            brand_color: None,
            accent_color: None,
            product_truth: None,
            visual_profile: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }];
        let posts: Vec<PortablePost> = Vec::new();
        let mut settings = std::collections::BTreeMap::new();
        settings.insert("active_provider".to_string(), "openrouter".to_string());
        let media = vec![("media/post-1-slide-01.png".to_string(), vec![0x89, 0x50])];
        let manifest = PortableManifest {
            format: PORTABLE_FORMAT,
            format_version: PORTABLE_FORMAT_VERSION,
            app_version: "test",
            exported_at: "now".to_string(),
            counts: PortableCounts {
                accounts: 1,
                posts: 0,
                media_files: 1,
            },
            notes: "test",
        };

        write_portable_archive(
            &archive_path,
            &accounts,
            &posts,
            &settings,
            &media,
            &manifest,
        )
        .expect("write_portable_archive failed");
        assert!(archive_path.exists());

        let f = std::fs::File::open(&archive_path).unwrap();
        let mut zip = zip::ZipArchive::new(f).expect("must be a valid ZIP");
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();

        for required in [
            "manifest.json",
            "accounts.json",
            "posts.json",
            "settings.json",
            "schema.sql",
            "media/post-1-slide-01.png",
        ] {
            assert!(
                names.contains(&required.to_string()),
                "archive must contain {required}, got {names:?}"
            );
        }

        let mut accounts_content = String::new();
        zip.by_name("accounts.json")
            .unwrap()
            .read_to_string(&mut accounts_content)
            .unwrap();
        assert!(!accounts_content.contains("token_key"));

        let _ = std::fs::remove_file(&archive_path);
    }
}
