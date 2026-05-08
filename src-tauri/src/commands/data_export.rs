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
    // 1. Resolve the output path. Fixed location keeps the UI simple
    //    (no save dialog needed = no extra plugin dep). The user can
    //    later move the file wherever they want.
    let downloads = dirs::download_dir()
        .ok_or_else(|| "Impossible de trouver le dossier Téléchargements".to_string())?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_path = downloads.join(format!("getpostcraft-backup-{timestamp}.gpcbak"));

    // 2. Snapshot the live DB. VACUUM INTO requires a literal path —
    //    parameter binding is not supported in this statement (sqlite
    //    parses the path at compile time, before any binding). Path
    //    comes from `temp_dir()` + a deterministic timestamped name,
    //    no user input ever flows in here, so format-string injection
    //    is structurally impossible.
    let temp_db = std::env::temp_dir().join(format!("getpostcraft-snapshot-{timestamp}.db"));
    // SQLite on Windows accepts forward slashes; normalising avoids
    // backslash escaping headaches in the SQL literal.
    let path_for_sql = temp_db
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''"); // defense in depth even though path is system-controlled

    sqlx::query(&format!("VACUUM INTO '{path_for_sql}'"))
        .execute(&state.db)
        .await
        .map_err(|e| format!("VACUUM INTO failed: {e}"))?;

    // 3. Read the snapshot, then delete the temp file immediately —
    //    we don't want the snapshot lingering in /tmp where other
    //    processes could read it.
    let db_bytes = std::fs::read(&temp_db).map_err(|e| format!("Read snapshot: {e}"))?;
    let _ = std::fs::remove_file(&temp_db);

    // 4. Manifest with checksum computed over the DB bytes that will
    //    actually land in the archive — verifies post-extraction.
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

    // 5. Write the ZIP. Deflate the .db (mostly text, compresses well),
    //    flat for the tiny manifest.
    write_archive(&output_path, &db_bytes, &manifest_json)?;

    Ok(output_path.to_string_lossy().to_string())
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
}
