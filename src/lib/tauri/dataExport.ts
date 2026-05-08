import { invoke } from "@tauri-apps/api/core";

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
