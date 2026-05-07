import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

/**
 * Updater workflow state — surfaced to UI so the user always sees what's happening.
 * Errors are stringified once at the boundary so React renders them safely.
 */
export type UpdaterStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up-to-date" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "downloading"; version: string; downloaded: number; total: number | null }
  | { kind: "installing"; version: string }
  | { kind: "ready"; version: string }
  | { kind: "error"; message: string };

/** Append a message to the in-memory log so issues are inspectable from the UI. */
export type LogSink = (level: "info" | "warn" | "error", message: string) => void;

/**
 * Check for an update without downloading. Returns the Update handle if one exists,
 * `null` otherwise. Errors are propagated as Error so callers can format them.
 */
export async function checkForUpdate(log: LogSink): Promise<Update | null> {
  try {
    log("info", "updater: checking endpoint");
    const update = await check();
    if (update) {
      log("info", `updater: update available v${update.version}`);
    } else {
      log("info", "updater: app is up to date");
    }
    return update;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    log("error", `updater: check failed — ${msg}`);
    throw e instanceof Error ? e : new Error(msg);
  }
}

/**
 * Download + install + relaunch. The updater plugin verifies the bundle signature
 * with the Ed25519 public key embedded in tauri.conf.json — invalid signatures abort.
 *
 * `onProgress` is called with cumulative bytes downloaded so the caller can show a
 * progress bar. `total` is null when the server does not advertise a content-length.
 */
export async function downloadAndInstall(
  update: Update,
  log: LogSink,
  onProgress: (downloaded: number, total: number | null) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | null = null;
  try {
    log("info", `updater: downloading v${update.version}`);
    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? null;
          log("info", `updater: started download (${total ?? "unknown"} bytes)`);
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          onProgress(downloaded, total);
          break;
        case "Finished":
          log("info", "updater: download finished, installing");
          break;
      }
    });
    log("info", "updater: install complete, relaunching");
    await relaunch();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    log("error", `updater: install failed — ${msg}`);
    throw e instanceof Error ? e : new Error(msg);
  }
}

/** Current app version, read from Tauri at runtime so it's always in sync with the bundle. */
export function appVersion(): Promise<string> {
  return getVersion();
}
