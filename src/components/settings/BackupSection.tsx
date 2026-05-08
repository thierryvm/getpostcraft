import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Download, FolderOpen, Loader2 } from "lucide-react";
import { exportBackupZip } from "@/lib/tauri/dataExport";

/**
 * Settings section that lets the user export a `.gpcbak` backup of their
 * local SQLite database to the Downloads folder. The format is documented
 * in `src-tauri/src/commands/data_export.rs` — anyone with `sqlite3` can
 * open the contained `app.db` directly, which is the explicit anti-lock-in
 * promise.
 */
export function BackupSection() {
  const [busy, setBusy] = useState(false);
  const [exportedPath, setExportedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleExport() {
    setBusy(true);
    setError(null);
    try {
      const path = await exportBackupZip();
      setExportedPath(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
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

  return (
    <div className="space-y-4">
      <div className="text-sm text-muted-foreground space-y-2">
        <p>
          Crée une archive <code className="px-1 py-0.5 rounded bg-muted text-xs">.gpcbak</code> contenant
          un instantané de ta base de données locale. L'archive est un ZIP
          standard que tu peux ouvrir avec n'importe quel outil et explorer
          directement avec <code className="px-1 py-0.5 rounded bg-muted text-xs">sqlite3 app.db</code>.
        </p>
        <p>
          <span className="font-medium text-foreground">Inclus :</span> tous les
          comptes connectés, brouillons, posts publiés, paramètres, branding,
          ProductTruth.
        </p>
        <p>
          <span className="font-medium text-foreground">Exclu volontairement :</span> tes
          clés API et tokens OAuth — ils restent dans le trousseau système et
          ne quittent jamais la machine. Sur un nouveau PC, recolle simplement
          ta clé API et reconnecte tes comptes.
        </p>
      </div>

      <div className="flex items-center gap-3 flex-wrap">
        <Button onClick={handleExport} disabled={busy} className="gap-2">
          {busy ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Download className="h-4 w-4" />
          )}
          {busy ? "Export en cours…" : "Exporter une sauvegarde"}
        </Button>

        {exportedPath && !busy && (
          <Button
            variant="ghost"
            onClick={revealInFolder}
            className="gap-2 text-muted-foreground"
          >
            <FolderOpen className="h-4 w-4" />
            Ouvrir le dossier
          </Button>
        )}
      </div>

      {exportedPath && !error && (
        <div className="text-xs rounded border border-border bg-muted/30 p-3 space-y-1">
          <p className="text-foreground font-medium">Sauvegarde créée :</p>
          <p className="font-mono break-all text-muted-foreground">{exportedPath}</p>
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
