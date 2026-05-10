import { invoke } from "./invoke";

/**
 * Exports a snapshot of the local SQLite database as a `.gpcbak` ZIP
 * archive saved to the user's Downloads folder.
 *
 * The archive contains:
 *   - `app.db` — a transactionally-consistent SQLite snapshot (VACUUM INTO)
 *   - `manifest.json` — format version, app version, ISO 8601 export
 *     timestamp, SHA-256 checksum
 *
 * Secrets (AI keys, OAuth tokens) are NOT included — they live in the OS
 * keyring and re-bind on a fresh install per the security boundary
 * established by PR-A and PR-X.
 *
 * Returns the absolute path of the created archive so the UI can show
 * a "Reveal in folder" affordance.
 */
export async function exportBackupZip(): Promise<string> {
  return invoke<string>("export_backup_zip");
}

/**
 * Exports the user's data in a multi-tool, portable format suitable for
 * loading into Supabase, vanilla Postgres, n8n, or any other tool that
 * speaks JSON. Saved to the Downloads folder.
 *
 * Archive contents:
 *   - `accounts.json`, `posts.json`, `settings.json` — flat JSON tables
 *   - `media/*.png` — base64 images decoded to actual PNG files
 *   - `schema.sql` — Postgres CREATE TABLEs (Supabase-ready)
 *   - `manifest.json` — format identifier + counts + restore notes
 *
 * Secrets are excluded for the same reason as `exportBackupZip`.
 *
 * Returns the absolute path of the created archive.
 */
export async function exportPortableZip(): Promise<string> {
  return invoke<string>("export_portable_zip");
}

/** Backup file metadata as returned by `list_auto_backups` / `get_restore_offer`. */
export interface BackupInfo {
  path: string;
  /** ISO 8601 UTC of the file modification time. */
  exported_at: string;
  /** Days elapsed between the backup and now. */
  age_days: number;
  size_bytes: number;
}

export interface RestoreOffer {
  backup: BackupInfo;
}

/**
 * Returns a restore offer if the database is fresh AND a backup exists in
 * `~/Documents/Getpostcraft/backups/`. The renderer mounts a one-shot
 * dialog at startup that calls this and decides whether to prompt the user.
 */
export async function getRestoreOffer(): Promise<RestoreOffer | null> {
  return invoke<RestoreOffer | null>("get_restore_offer");
}

/** Lists every auto-backup currently on disk, newest first. */
export async function listAutoBackups(): Promise<BackupInfo[]> {
  return invoke<BackupInfo[]>("list_auto_backups");
}

/**
 * Restore a `.gpcbak` archive over the live database. Validates the
 * manifest format and SHA-256 checksum before touching anything.
 *
 * The caller MUST trigger a process restart after this resolves — sqlx
 * is still pointing at the old file's handle and serving stale data
 * until the app is relaunched.
 */
export async function importBackupZip(path: string): Promise<void> {
  return invoke<void>("import_backup_zip", { path });
}
