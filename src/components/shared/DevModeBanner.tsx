import { AlertTriangle } from "lucide-react";
import { isInTauriContext } from "@/lib/tauri/invoke";

/**
 * Shown once at the app root when the renderer is hosted by Vite's dev server
 * (`npm run dev`) instead of the Tauri WebView. In that mode every IPC call
 * fails because `__TAURI_INTERNALS__` is undefined; without this banner the
 * user has to scroll through cryptic per-panel TypeError messages to figure
 * out why nothing loads.
 *
 * The banner renders nothing in production (Tauri WebView always exposes
 * the global), so the cost in the shipped binary is one boolean check at
 * mount time.
 */
export function DevModeBanner() {
  if (isInTauriContext()) return null;

  return (
    <div
      role="status"
      aria-live="polite"
      className="flex items-start gap-2 border-b border-orange-500/40 bg-orange-500/10 px-4 py-2 text-xs text-orange-200"
    >
      <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
      <div className="flex flex-col gap-0.5">
        <span className="font-medium">Mode dev sans runtime Tauri</span>
        <span className="text-orange-200/80">
          Les commandes IPC (génération IA, comptes, publication, calendrier,
          stats) renvoient une erreur. Lance{" "}
          <code className="rounded bg-orange-500/20 px-1 font-mono">npm run tauri dev</code>{" "}
          pour le runtime complet, ou installe l'app desktop. UI iteration only sinon.
        </span>
      </div>
    </div>
  );
}
