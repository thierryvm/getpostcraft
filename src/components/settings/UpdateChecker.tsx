import { useEffect, useRef, useState } from "react";
import { Download, RefreshCw, CheckCircle2, AlertCircle, Loader2 } from "lucide-react";
import type { Update } from "@tauri-apps/plugin-updater";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  appVersion,
  checkForUpdate,
  downloadAndInstall,
  type LogSink,
  type UpdaterStatus,
} from "@/lib/tauri/updater";

interface LogEntry {
  ts: string;
  level: "info" | "warn" | "error";
  message: string;
}

function statusBadge(status: UpdaterStatus): { label: string; tone: "neutral" | "primary" | "destructive" } {
  switch (status.kind) {
    case "idle":
      return { label: "Inactif", tone: "neutral" };
    case "checking":
      return { label: "Vérification…", tone: "neutral" };
    case "up-to-date":
      return { label: "À jour", tone: "primary" };
    case "available":
      return { label: `v${status.version} disponible`, tone: "primary" };
    case "downloading":
      return { label: "Téléchargement…", tone: "neutral" };
    case "installing":
      return { label: "Installation…", tone: "neutral" };
    case "ready":
      return { label: "Redémarrage…", tone: "primary" };
    case "error":
      return { label: "Erreur", tone: "destructive" };
  }
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function UpdateChecker() {
  const [status, setStatus] = useState<UpdaterStatus>({ kind: "idle" });
  const [version, setVersion] = useState<string>("…");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const logsRef = useRef<HTMLDivElement>(null);
  // Hold the Update handle from the last `checkForUpdate` so install uses
  // the exact version the user saw in the UI. Re-checking inside `handleInstall`
  // would race with a publish/retraction between the two calls and either
  // silently abort or download a different version than what was confirmed.
  const pendingUpdate = useRef<Update | null>(null);

  const log: LogSink = (level, message) => {
    const entry: LogEntry = {
      ts: new Date().toLocaleTimeString(),
      level,
      message,
    };
    setLogs((prev) => [...prev.slice(-49), entry]);
    // Forward to the Rust log file too so issues are inspectable from Settings → Logs.
    const consoleFn = level === "error" ? console.error : level === "warn" ? console.warn : console.info;
    consoleFn(`[updater] ${message}`);
  };

  useEffect(() => {
    appVersion()
      .then(setVersion)
      .catch(() => setVersion("inconnu"));
  }, []);

  // Auto-scroll log on new entries
  useEffect(() => {
    if (logsRef.current) {
      logsRef.current.scrollTop = logsRef.current.scrollHeight;
    }
  }, [logs.length]);

  const handleCheck = async () => {
    setStatus({ kind: "checking" });
    pendingUpdate.current = null;
    try {
      const update = await checkForUpdate(log);
      if (update) {
        pendingUpdate.current = update;
        setStatus({
          kind: "available",
          version: update.version,
          notes: update.body ?? null,
        });
      } else {
        setStatus({ kind: "up-to-date" });
      }
    } catch (e) {
      setStatus({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleInstall = async () => {
    if (status.kind !== "available") return;
    const update = pendingUpdate.current;
    if (!update) {
      // Either the user never clicked "Vérifier" or the handle was cleared.
      // Force a fresh check rather than silently downloading whatever is now
      // pinned as latest — the user explicitly approved the version they saw.
      setStatus({ kind: "up-to-date" });
      return;
    }
    setStatus({
      kind: "downloading",
      version: status.version,
      downloaded: 0,
      total: null,
    });
    try {
      await downloadAndInstall(update, log, (downloaded, total) => {
        setStatus((s) =>
          s.kind === "downloading"
            ? { kind: "downloading", version: s.version, downloaded, total }
            : s,
        );
      });
      setStatus({ kind: "ready", version: status.version });
    } catch (e) {
      setStatus({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const badge = statusBadge(status);
  const isBusy =
    status.kind === "checking" ||
    status.kind === "downloading" ||
    status.kind === "installing";

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border p-4">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">Mises à jour</span>
          <span className="text-xs text-muted-foreground">
            Version installée : <span className="font-mono">v{version}</span>
          </span>
        </div>
        <Badge
          variant={badge.tone === "destructive" ? "destructive" : badge.tone === "primary" ? "default" : "secondary"}
          className="text-xs"
        >
          {badge.label}
        </Badge>
      </div>

      {/* Update notes */}
      {status.kind === "available" && status.notes && (
        <div className="rounded-md bg-secondary/50 px-3 py-2 text-xs text-muted-foreground whitespace-pre-line max-h-32 overflow-y-auto">
          {status.notes}
        </div>
      )}

      {/* Progress */}
      {status.kind === "downloading" && (
        <div className="flex flex-col gap-1">
          <div className="h-1.5 w-full overflow-hidden rounded bg-secondary">
            <div
              className="h-full bg-primary transition-[width] duration-150"
              style={{
                width: status.total
                  ? `${Math.min(100, (status.downloaded / status.total) * 100)}%`
                  : "20%",
              }}
            />
          </div>
          <span className="text-xs text-muted-foreground">
            {formatBytes(status.downloaded)}
            {status.total ? ` / ${formatBytes(status.total)}` : ""}
          </span>
        </div>
      )}

      {/* Error */}
      {status.kind === "error" && (
        <div className="flex items-start gap-2 rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
          <AlertCircle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
          <span className="font-mono break-all">{status.message}</span>
        </div>
      )}

      {/* Action buttons */}
      <div className="flex gap-2">
        <Button
          variant="outline"
          size="sm"
          className="gap-1.5"
          disabled={isBusy}
          onClick={handleCheck}
        >
          {status.kind === "checking" ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <RefreshCw className="h-3.5 w-3.5" />
          )}
          Vérifier
        </Button>
        {status.kind === "available" && (
          <Button size="sm" className="gap-1.5" onClick={handleInstall}>
            <Download className="h-3.5 w-3.5" />
            Installer v{status.version}
          </Button>
        )}
        {status.kind === "up-to-date" && (
          <span className="flex items-center gap-1.5 text-xs text-primary">
            <CheckCircle2 className="h-3.5 w-3.5" />
            Aucune mise à jour disponible
          </span>
        )}
      </div>

      {/* Console log — inline for quick diagnosis without leaving Settings */}
      {logs.length > 0 && (
        <details className="text-xs">
          <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
            Logs ({logs.length})
          </summary>
          <div
            ref={logsRef}
            className="mt-2 max-h-40 overflow-y-auto rounded bg-secondary/30 p-2 font-mono"
          >
            {logs.map((entry, i) => (
              <div
                key={i}
                className={
                  entry.level === "error"
                    ? "text-destructive"
                    : entry.level === "warn"
                    ? "text-orange-400"
                    : "text-muted-foreground"
                }
              >
                <span className="opacity-60">{entry.ts}</span> {entry.message}
              </div>
            ))}
          </div>
        </details>
      )}
    </div>
  );
}
