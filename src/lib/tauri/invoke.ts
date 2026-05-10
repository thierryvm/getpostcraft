import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/**
 * Marker error type the UI can pattern-match on to swap a destructive red
 * "TypeError" for a friendly "Tauri runtime not available — open the
 * desktop app" message. Components that hit this in dev mode (`npm run
 * dev` without `npm run tauri dev`) shouldn't read the underlying
 * cause as a real bug.
 */
export class TauriRuntimeUnavailableError extends Error {
  readonly kind = "tauri-runtime-unavailable" as const;
  constructor() {
    super(
      "Tauri runtime indisponible — lance l'application desktop (ou `npm run tauri dev`) " +
        "au lieu d'un dev server pur (`npm run dev`)",
    );
    this.name = "TauriRuntimeUnavailableError";
  }
}

/**
 * True when this renderer is hosted inside a Tauri WebView (the binary that
 * ships with the desktop app), false in a plain browser served by Vite's
 * dev server. We detect via the runtime-injected `__TAURI_INTERNALS__`
 * global — Tauri 2 puts it on `window` early in the boot sequence before
 * any user JS runs.
 */
export function isInTauriContext(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in (window as object);
}

/**
 * Drop-in wrapper around `@tauri-apps/api/core`'s `invoke` that throws a
 * typed `TauriRuntimeUnavailableError` instead of the cryptic
 * `TypeError: Cannot read properties of undefined (reading 'invoke')`
 * when no Tauri runtime is wired up.
 *
 * Use this in every IPC wrapper under `src/lib/tauri/*` so panels can
 * pattern-match the error in dev mode and render a friendly placeholder
 * instead of a destructive red error block.
 */
export function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isInTauriContext()) {
    return Promise.reject(new TauriRuntimeUnavailableError());
  }
  return tauriInvoke<T>(cmd, args);
}
