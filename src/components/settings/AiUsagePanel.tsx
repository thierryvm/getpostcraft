import { useQuery } from "@tanstack/react-query";
import { Badge } from "@/components/ui/badge";
import { Loader2, TrendingUp } from "lucide-react";
import { getAiUsageSummary } from "@/lib/tauri/settings";

/**
 * Settings → IA panel that shows the user's BYOK spend.
 *
 * Cost is computed in Rust at query time from token counts stored in the
 * `ai_usage` table — see `src-tauri/src/db/ai_usage.rs`. We don't store
 * cost directly so a future price update re-prices history automatically.
 *
 * Numbers are advisory: the `price_estimated` flag surfaces the cases
 * where the model wasn't in our pricing map (typically a freshly added
 * OpenRouter model). The user shouldn't treat this as billing-grade
 * accounting — the provider's own dashboard remains the source of truth.
 */
export function AiUsagePanel() {
  const { data, isLoading, error } = useQuery({
    queryKey: ["ai-usage-summary"],
    queryFn: getAiUsageSummary,
    // Refetch when the user generates content so the table stays live.
    staleTime: 30_000,
  });

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        Chargement de l'historique d'usage…
      </div>
    );
  }

  if (error) {
    return (
      <p className="text-sm text-destructive bg-destructive/10 rounded p-2">
        Erreur : {String(error)}
      </p>
    );
  }

  if (!data) return null;

  const hasData = data.calls_30d > 0;

  return (
    <div className="space-y-4">
      <p className="text-xs text-muted-foreground">
        Compteur de tokens basé sur ce que les SDKs renvoient. Coût calculé
        depuis une table de prix interne — c'est <em>indicatif</em>, ton
        dashboard fournisseur reste la source de vérité.
      </p>

      {/* Top-line stats */}
      <div className="grid grid-cols-3 gap-3">
        <Stat label="Appels (30 j)" value={data.calls_30d.toString()} />
        <Stat label="Coût 30 j" value={formatUsd(data.cost_usd_30d)} />
        <Stat
          label="Coût mois en cours"
          value={formatUsd(data.cost_usd_month)}
          accent
        />
      </div>

      {/* By-model breakdown */}
      {hasData ? (
        <div className="rounded border border-border overflow-hidden">
          <table className="w-full text-xs">
            <thead className="bg-muted/30">
              <tr className="text-left">
                <th className="px-3 py-2 font-medium text-muted-foreground">Modèle</th>
                <th className="px-3 py-2 font-medium text-muted-foreground text-right">Appels</th>
                <th className="px-3 py-2 font-medium text-muted-foreground text-right">Tokens in</th>
                <th className="px-3 py-2 font-medium text-muted-foreground text-right">Tokens out</th>
                <th className="px-3 py-2 font-medium text-muted-foreground text-right">Coût</th>
              </tr>
            </thead>
            <tbody>
              {data.by_model_30d.map((m) => (
                <tr key={`${m.provider}/${m.model}`} className="border-t border-border/50">
                  <td className="px-3 py-2 font-mono text-foreground/90">
                    <div className="flex items-center gap-2">
                      <span className="truncate max-w-[280px]">{m.model}</span>
                      {m.price_estimated && (
                        <Badge variant="outline" className="text-[9px] px-1 py-0 border-amber-500/30 text-amber-400">
                          estimé
                        </Badge>
                      )}
                    </div>
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">{m.calls}</td>
                  <td className="px-3 py-2 text-right tabular-nums text-muted-foreground">
                    {m.input_tokens.toLocaleString("fr-FR")}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums text-muted-foreground">
                    {m.output_tokens.toLocaleString("fr-FR")}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums text-foreground">
                    {formatUsd(m.cost_usd)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="flex items-center gap-2 text-sm text-muted-foreground rounded border border-dashed border-border p-4">
          <TrendingUp className="h-4 w-4" />
          Pas encore d'appel IA enregistré sur les 30 derniers jours.
        </div>
      )}
    </div>
  );
}

function Stat({
  label,
  value,
  accent = false,
}: {
  label: string;
  value: string;
  accent?: boolean;
}) {
  // Accent uses the design-system `primary` token (`#3ddc84`) instead of
  // raw `emerald-*` so a future theme change cascades automatically and
  // the highlighted stat matches the brand colour everywhere else
  // (badges, links, focus rings).
  return (
    <div className={`rounded border ${accent ? "border-primary/30 bg-primary/5" : "border-border bg-muted/20"} p-3`}>
      <p className="text-[10px] uppercase tracking-wide text-muted-foreground">{label}</p>
      <p className={`text-lg font-semibold tabular-nums ${accent ? "text-primary" : "text-foreground"}`}>
        {value}
      </p>
    </div>
  );
}

/**
 * Format USD with up to 4 decimal places when sub-cent precision matters
 * (one Haiku call can be ~$0.0001), but cap at 2 decimals once the figure
 * exceeds $1 to keep the UI tidy.
 */
function formatUsd(n: number): string {
  if (n === 0) return "$0";
  if (n < 1) return `$${n.toFixed(4).replace(/\.?0+$/, "")}`;
  return `$${n.toFixed(2)}`;
}
