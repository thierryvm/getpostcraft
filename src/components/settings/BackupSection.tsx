import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { AlertTriangle, Database, FileJson, FolderOpen, Loader2 } from "lucide-react";
import { exportBackupZip, exportPortableZip } from "@/lib/tauri/dataExport";

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
      await invoke("plugin:opener|reveal_item_in_dir", { path: exportedPath });
    } catch {
      // Fallback: open the parent folder if reveal_item_in_dir is unsupported
      // by the current opener plugin version.
      const parent = exportedPath.replace(/[\\/][^\\/]+$/, "");
      try {
        await invoke("plugin:opener|open_path", { path: parent });
      } catch (e) {
        console.error("Cannot open folder:", e);
      }
    }
  }

  const busy = busyKind !== null;

  return (
    <div className="space-y-5">
      {/* Pre-uninstall warning — the Windows NSIS uninstaller offers
          "Remove user data" enabled by default, which wipes app.db AND the
          OS keychain entries. Make the safety net visible. */}
      <div className="rounded border border-amber-500/30 bg-amber-500/10 p-3 flex items-start gap-3">
        <AlertTriangle className="h-4 w-4 mt-0.5 text-amber-400 shrink-0" />
        <div className="text-xs text-amber-100/90 space-y-1">
          <p className="font-medium text-amber-300">Avant de désinstaller : exporte une sauvegarde</p>
          <p>
            L'option « Remove user data » du désinstalleur Windows efface ta
            base locale ET tes clés du trousseau système (impossible à
            récupérer). Un export <code className="px-1 py-0.5 rounded bg-black/20">.gpcbak</code>{" "}
            te permet de tout restaurer.
          </p>
        </div>
      </div>

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
