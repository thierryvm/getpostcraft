import { useEffect, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  getRestoreOffer,
  importBackupZip,
  type RestoreOffer,
} from "@/lib/tauri/dataExport";

/**
 * One-shot dialog that surfaces on app boot when the database is fresh
 * AND a `.gpcbak` is sitting in `~/Documents/Getpostcraft/backups/`.
 * That state means a recent install / reinstall on a machine that
 * already had the app — and we offer to restore from the most recent
 * auto-backup before the user even notices the data is gone.
 *
 * The user can decline; we set `localStorage[SKIP_KEY]` so the dialog
 * doesn't pester them again on subsequent launches if they really did
 * mean to start fresh.
 *
 * On accept, we replace `app.db` and call `relaunch()` from the process
 * plugin — sqlx's pool is still pointing at the old inode at this
 * point, so a process restart is the cheapest way to pick up the new
 * DB cleanly.
 */

/** localStorage flag — when "true", we don't show the prompt for the same backup path again. */
const RESTORE_SKIP_KEY = "gpc.restorePrompt.skipped";

export function RestorePromptDialog() {
  const [offer, setOffer] = useState<RestoreOffer | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await getRestoreOffer();
        if (cancelled || !result) return;
        // Honour a previous "skip" only when it was for THIS exact backup.
        // A new backup file (different timestamp) means we should re-ask
        // — the user dismissed once, we shouldn't suppress forever.
        const skipped = localStorage.getItem(RESTORE_SKIP_KEY);
        if (skipped && skipped === result.backup.path) return;
        setOffer(result);
      } catch (e) {
        // Silent: the prompt is best-effort. If the IPC fails, the user
        // can still restore manually via Settings → Données. Log only.
        console.warn("getRestoreOffer failed:", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (!offer) return null;

  const dt = new Date(offer.backup.exported_at);
  const dtLabel = isNaN(dt.getTime())
    ? offer.backup.exported_at
    : dt.toLocaleString("fr-FR", {
        day: "2-digit",
        month: "long",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
  const sizeMb = (offer.backup.size_bytes / (1024 * 1024)).toFixed(1);

  async function handleRestore() {
    if (!offer) return;
    setBusy(true);
    setError(null);
    try {
      await importBackupZip(offer.backup.path);
      // Restart so sqlx re-opens the swapped DB cleanly. The OS cache
      // for the old inode goes away with the process.
      await relaunch();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  function handleSkip() {
    if (!offer) return;
    try {
      // Tie the skip to the specific backup path so a future, newer
      // backup re-triggers the prompt instead of staying suppressed.
      localStorage.setItem(RESTORE_SKIP_KEY, offer.backup.path);
    } catch {
      // localStorage disabled — accept the in-memory dismiss only.
    }
    setOffer(null);
  }

  return (
    <AlertDialog open={offer !== null}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Sauvegarde récente détectée</AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="space-y-2 text-sm text-muted-foreground">
              <p>
                Ta base de données est vide mais une sauvegarde existe dans
                <code className="px-1 py-0.5 mx-1 rounded bg-muted text-xs">
                  Documents/Getpostcraft/backups/
                </code>
                . Tu veux la restaurer ?
              </p>
              <ul className="text-xs space-y-0.5 pl-2 border-l-2 border-border">
                <li>
                  <span className="text-foreground">Date :</span> {dtLabel}{" "}
                  <span className="text-muted-foreground">
                    ({offer.backup.age_days} {offer.backup.age_days === 1 ? "jour" : "jours"})
                  </span>
                </li>
                <li>
                  <span className="text-foreground">Taille :</span> {sizeMb} Mo
                </li>
              </ul>
              <p className="text-xs">
                Ce qui sera restauré : posts, comptes connectés (métadonnées),
                ProductTruth, branding, paramètres. Les clés API et tokens
                OAuth ne sont pas dans la sauvegarde — il faudra les recoller
                / refaire l'OAuth ensuite.
              </p>
              {error && (
                <p className="text-xs text-destructive bg-destructive/10 rounded p-2">
                  Échec : {error}
                </p>
              )}
            </div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy} onClick={handleSkip}>
            Démarrer à vide
          </AlertDialogCancel>
          <AlertDialogAction disabled={busy} onClick={handleRestore}>
            {busy ? "Restauration…" : "Restaurer"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
