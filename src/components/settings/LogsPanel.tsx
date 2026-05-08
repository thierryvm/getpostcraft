import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { RefreshCw, Copy, FolderOpen, Trash2 } from "lucide-react";

interface LogEntry {
  level: "info" | "warn" | "error" | "debug";
  timestamp: string;
  message: string;
}

type LevelFilter = "all" | "info" | "warn" | "error";

const LEVEL_COLORS: Record<string, string> = {
  error: "text-destructive",
  warn:  "text-orange-400",
  info:  "text-muted-foreground",
  debug: "text-muted-foreground/50",
};

const LEVEL_BADGE: Record<string, string> = {
  error: "bg-destructive/20 text-destructive border-destructive/30",
  warn:  "bg-orange-400/20 text-orange-400 border-orange-400/30",
  info:  "bg-muted text-muted-foreground border-border",
  debug: "bg-muted/50 text-muted-foreground/50 border-border/50",
};

export function LogsPanel() {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState<LevelFilter>("all");
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      const logs = await invoke<LogEntry[]>("get_app_logs", { lines: 300 });
      setEntries(logs);
      setLoaded(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function openLogFile() {
    try {
      const path = await invoke<string>("get_log_file_path");
      await invoke("plugin:opener|open_path", { path });
    } catch (e) {
      console.error("Cannot open log file:", e);
    }
  }

  function copyToClipboard() {
    const text = filtered
      .map((e) => `[${e.level.toUpperCase()}] ${e.timestamp} ${e.message}`)
      .join("\n");
    navigator.clipboard.writeText(text).catch(() => {});
  }

  const filtered =
    filter === "all"
      ? entries
      : entries.filter((e) => e.level === filter);

  const counts = {
    error: entries.filter((e) => e.level === "error").length,
    warn:  entries.filter((e) => e.level === "warn").length,
    info:  entries.filter((e) => e.level === "info").length,
  };

  return (
    <div className="space-y-3">
      {/* Toolbar */}
      <div className="flex items-center gap-2 flex-wrap">
        <Button
          size="sm"
          variant="outline"
          onClick={load}
          disabled={loading}
          className="gap-1.5"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
          {loaded ? "Rafraîchir" : "Charger les logs"}
        </Button>

        {loaded && (
          <>
            <div className="flex gap-1">
              {(["all", "error", "warn", "info"] as LevelFilter[]).map((lvl) => (
                <button
                  key={lvl}
                  onClick={() => setFilter(lvl)}
                  className={`px-2 py-0.5 rounded text-xs border transition-colors ${
                    filter === lvl
                      ? "bg-accent text-accent-foreground border-accent"
                      : "border-border text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {lvl === "all" ? `Tout (${entries.length})` : lvl === "error" ? `Erreurs (${counts.error})` : lvl === "warn" ? `Avertissements (${counts.warn})` : `Info (${counts.info})`}
                </button>
              ))}
            </div>

            <div className="ml-auto flex gap-1.5">
              <Button size="sm" variant="ghost" onClick={copyToClipboard} className="gap-1.5 h-7 px-2">
                <Copy className="h-3 w-3" />
                Copier
              </Button>
              <Button size="sm" variant="ghost" onClick={openLogFile} className="gap-1.5 h-7 px-2">
                <FolderOpen className="h-3 w-3" />
                Ouvrir fichier
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => { setEntries([]); setLoaded(false); }}
                aria-label="Effacer les logs affichés"
                className="gap-1.5 h-7 px-2 text-muted-foreground"
              >
                <Trash2 className="h-3 w-3" aria-hidden="true" />
              </Button>
            </div>
          </>
        )}
      </div>

      {error && (
        <p className="text-sm text-destructive bg-destructive/10 rounded p-2">{error}</p>
      )}

      {/* Log list */}
      {loaded && (
        <div className="rounded border border-border bg-[#0d1117] font-mono text-xs overflow-auto max-h-[480px]">
          {filtered.length === 0 ? (
            <p className="p-4 text-muted-foreground text-center">Aucune entrée pour ce filtre.</p>
          ) : (
            <table className="w-full">
              <tbody>
                {filtered.map((entry, i) => (
                  <tr
                    key={i}
                    className="border-b border-border/30 hover:bg-white/5 transition-colors"
                  >
                    <td className="pl-3 pr-2 py-1 w-14 whitespace-nowrap">
                      <Badge
                        variant="outline"
                        className={`text-[10px] px-1 py-0 ${LEVEL_BADGE[entry.level] ?? LEVEL_BADGE.info}`}
                      >
                        {entry.level.toUpperCase()}
                      </Badge>
                    </td>
                    <td className="pr-3 py-1 text-muted-foreground/60 whitespace-nowrap w-40">
                      {entry.timestamp}
                    </td>
                    <td className={`pr-3 py-1 break-all ${LEVEL_COLORS[entry.level] ?? ""}`}>
                      {entry.message}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {!loaded && !loading && (
        <p className="text-sm text-muted-foreground">
          Cliquez sur "Charger les logs" pour afficher les événements de l'application.
          Le fichier est stocké dans le répertoire de données de l'app (~5 Mo max, 3 rotations).
        </p>
      )}
    </div>
  );
}
