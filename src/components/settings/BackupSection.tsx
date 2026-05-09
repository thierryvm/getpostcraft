import { useState } from "react";
import { revealItemInDir, openPath } from "@tauri-apps/plugin-opener";
import { Button } from "@/components/ui/button";
import { AlertTriangle, Database, FileJson, FolderOpen, Loader2, X } from "lucide-react";
import { exportBackupZip, exportPortableZip } from "@/lib/tauri/dataExport";

/** localStorage key for the dismiss-state of the pre-uninstall banner. */
const UNINSTALL_BANNER_DISMISSED_KEY = "gpc.uninstallWarning.dismissed";

/**
 * The "Remove user data" trap is a Windows NSIS-uninstaller behaviour. macOS
 * `.app` and Linux .deb / AppImage uninstalls have no equivalent prompt — they
 * leave the user data dir alone. Showing the banner there would only confuse.
 *
 * We sniff `navigator.userAgent` rather than calling the Tauri `os` plugin to
 * avoid the extra permission/dependency: this is a UX-only check, not a
 * security gate, and userAgent is reliable enough for "is this a Windows
 * webview".
 */
function isWindowsHost(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Windows/i.test(navigator.userAgent);
}

/**
 * Settings section that lets the user export their local data in two
 * complementary formats:
 *
 *   - `.gpcbak` (backup): a ZIP wrapping the live SQLite database. Best for
 *     restoring on another GPC install or auditing with `sqlite3`.
 *   - `.zip` (portable): JSON tables + decoded media + Postgres `schema.sql`.
 *     Best for migrating to Supabase, Postgres, n8n, or any tool outside
 *     the GPC ecosystem.
 *
 * Both formats deliberately omit secrets; see `src-tauri/src/commands/data_export.rs`.
 */
export function BackupSection() {
  const [busyKind, setBusyKind] = useState<"backup" | "portable" | null>(null);
  const [exportedPath, setExportedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Banner is Windows-only (NSIS-specific issue) and dismissible: once the
  // user has read it, they shouldn't keep paying vertical real estate every
  // visit to this tab. Dismiss state persists across launches via localStorage.
  const [bannerDismissed, setBannerDismissed] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem(UNINSTALL_BANNER_DISMISSED_KEY) === "true";
  });

  function dismissBanner() {
    setBannerDismissed(true);
    try {
      window.localStorage.setItem(UNINSTALL_BANNER_DISMISSED_KEY, "true");
    } catch {
      // localStorage can be disabled (private mode in some browsers); the
      // in-memory dismiss still hides the banner for this session.
    }
  }

  const showUninstallBanner = isWindowsHost() && !bannerDismissed;

  async function runExport(kind: "backup" | "portable") {
    setBusyKind(kind);
    setError(null);
    try {
      const path =
        kind === "backup" ? await exportBackupZip() : await exportPortableZip();
      setExportedPath(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyKind(null);
    }
  }

  async function revealInFolder() {
    if (!exportedPath) return;
    try {
      await revealItemInDir(exportedPath);
    } catch (firstErr) {
      // Fallback: open the parent folder if reveal-item-in-dir failed
      // (some Linux desktops don't implement it). The two failures are
      // wrapped together so the user gets context if both fail.
      const parent = exportedPath.replace(/[\\/][^\\/]+$/, "");
      try {
        await openPath(parent);
      } catch (secondErr) {
        setError(
          `Impossible d'ouvrir le dossier : ${String(secondErr)} (reveal: ${String(firstErr)})`,
        );
      }
    }
  }

  const busy = busyKind !== null;

  return (
    <div className="space-y-5">
      {/* Pre-uninstall warning — Windows-only because the NSIS "Remove user
          data" prompt is the source of the data-loss incident; macOS and
          Linux uninstalls don't have an equivalent. `role="alert"` so screen
          readers announce the warning instead of silently presenting it as
          a normal block. Dismissible — once the user has read it, the
          state persists in localStorage so subsequent visits don't waste
          vertical space. */}
      {showUninstallBanner && (
        <div
          role="alert"
          className="rounded border border-amber-500/30 bg-amber-500/10 p-3 flex items-start gap-3"
        >
          <AlertTriangle className="h-4 w-4 mt-0.5 text-amber-400 shrink-0" aria-hidden="true" />
          <div className="text-xs text-amber-100/90 space-y-1 flex-1">
            <p className="font-medium text-amber-300">
              Avant de désinstaller : exporte une sauvegarde
            </p>
            <p>
              L'option « Remove user data » du désinstalleur Windows efface ta
              base locale ET tes clés du trousseau système (impossible à
              récupérer). Un export{" "}
              <code className="px-1 py-0.5 rounded bg-black/20">.gpcbak</code>{" "}
              te permet de tout restaurer.
            </p>
          </div>
          <button
            type="button"
            onClick={dismissBanner}
            aria-label="Masquer cet avertissement"
            className="text-amber-400/70 hover:text-amber-300 transition-colors shrink-0"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      )}

      <p className="text-sm text-muted-foreground">
        <span className="font-medium text-foreground">Exclu volontairement</span> des
        deux formats : tes clés API et tokens OAuth. Ils restent dans le
        trousseau système et ne quittent jamais la machine. Sur un nouveau PC,
        recolle ta clé et reconnecte tes comptes.
      </p>

      {/* Backup .gpcbak */}
      <div className="rounded border border-border p-4 space-y-3">
        <div className="flex items-start gap-3">
          <Database className="h-4 w-4 mt-0.5 text-muted-foreground shrink-0" />
          <div className="flex-1 space-y-1">
            <h3 className="font-medium text-foreground text-sm">
              Sauvegarde complète <code className="px-1 py-0.5 rounded bg-muted text-xs">.gpcbak</code>
            </h3>
            <p className="text-xs text-muted-foreground">
              Snapshot fidèle de la base SQLite. Ouvre-le avec{" "}
              <code className="px-1 py-0.5 rounded bg-muted">sqlite3 app.db</code> ou n'importe
              quel outil ZIP. Idéal pour restaurer sur un autre poste.
            </p>
          </div>
        </div>
        <Button
          onClick={() => runExport("backup")}
          disabled={busy}
          className="gap-2"
          size="sm"
        >
          {busyKind === "backup" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Database className="h-4 w-4" />
          )}
          {busyKind === "backup" ? "Export en cours…" : "Exporter en .gpcbak"}
        </Button>
      </div>

      {/* Portable .zip */}
      <div className="rounded border border-border p-4 space-y-3">
        <div className="flex items-start gap-3">
          <FileJson className="h-4 w-4 mt-0.5 text-muted-foreground shrink-0" />
          <div className="flex-1 space-y-1">
            <h3 className="font-medium text-foreground text-sm">
              Format portable (Supabase / Postgres / n8n)
            </h3>
            <p className="text-xs text-muted-foreground">
              JSON par table + médias en PNG + <code className="px-1 py-0.5 rounded bg-muted">schema.sql</code>{" "}
              Postgres prêt-à-charger. Pour migrer vers Supabase ou tout outil
              externe à GPC.
            </p>
          </div>
        </div>
        <Button
          onClick={() => runExport("portable")}
          disabled={busy}
          variant="secondary"
          className="gap-2"
          size="sm"
        >
          {busyKind === "portable" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <FileJson className="h-4 w-4" />
          )}
          {busyKind === "portable" ? "Export en cours…" : "Exporter en .zip portable"}
        </Button>
      </div>

      {/* Result */}
      {exportedPath && !error && (
        <div className="text-xs rounded border border-border bg-muted/30 p-3 space-y-2">
          <p className="text-foreground font-medium">Archive créée :</p>
          <p className="font-mono break-all text-muted-foreground">{exportedPath}</p>
          <Button
            variant="ghost"
            onClick={revealInFolder}
            className="gap-2 text-muted-foreground h-7 px-2"
            size="sm"
          >
            <FolderOpen className="h-3.5 w-3.5" />
            Ouvrir le dossier
          </Button>
        </div>
      )}

      {error && (
        <p className="text-sm text-destructive bg-destructive/10 rounded p-2">
          Échec de l'export : {error}
        </p>
      )}
    </div>
  );
}
