import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Loader2, TrendingUp, RefreshCw } from "lucide-react";
import {
  getAiUsageSummary,
  getOpenRouterPricingSnapshot,
  refreshOpenRouterPricing,
} from "@/lib/tauri/settings";
import { TauriRuntimeUnavailableError } from "@/lib/tauri/invoke";
import { format, formatDistanceToNow } from "date-fns";
import { fr } from "date-fns/locale";

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
  const qc = useQueryClient();

  const { data, isLoading, error } = useQuery({
    queryKey: ["ai-usage-summary"],
    queryFn: getAiUsageSummary,
    // Refetch when the user generates content so the table stays live.
    staleTime: 30_000,
  });

  const { data: pricing } = useQuery({
    queryKey: ["openrouter-pricing-snapshot"],
    queryFn: getOpenRouterPricingSnapshot,
    // Static snapshot — only changes when the user clicks Refresh.
    staleTime: Number.POSITIVE_INFINITY,
  });

  const refresh = useMutation({
    mutationFn: refreshOpenRouterPricing,
    onSuccess: () => {
      // Invalidate both — the new pricing recalculates the cost summary.
      qc.invalidateQueries({ queryKey: ["openrouter-pricing-snapshot"] });
      qc.invalidateQueries({ queryKey: ["ai-usage-summary"] });
    },
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
    // Dev-server (no Tauri runtime) hits this on every mount. Render a
    // muted hint instead of a red error so dev-mode pages aren't littered
    // with destructive blocks. Real backend errors still get the
    // destructive treatment so they don't blend into the chrome.
    if (error instanceof TauriRuntimeUnavailableError) {
      return (
        <p className="text-xs text-muted-foreground bg-muted/30 rounded p-3">
          Données indisponibles en mode dev. Lance{" "}
          <code className="font-mono">npm run tauri dev</code>{" "}
          ou installe l'app desktop pour voir les coûts d'usage.
        </p>
      );
    }
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
        depuis les tarifs <strong>OpenRouter en direct</strong> (avec fallback
        sur une table interne quand le modèle n'est pas dans leur catalogue).
        Ton dashboard fournisseur reste la source de vérité pour la facturation.
      </p>

      {/* Pricing freshness banner — shows when OpenRouter rates were last
          pulled and lets the user trigger a manual refresh. The startup
          task already runs once 5s after launch; this button covers the
          "I just changed my model and want today's rate now" case. */}
      <div className="flex items-center justify-between gap-2 rounded-md border border-border bg-muted/30 px-3 py-2 text-xs">
        <div className="flex flex-col gap-0.5">
          <span className="text-foreground">
            Tarifs OpenRouter
            {pricing?.last_refreshed_at ? (
              <span className="ml-1 text-muted-foreground">
                · {Object.keys(pricing.prices).length} modèles ·
                rafraîchis {formatDistanceToNow(new Date(pricing.last_refreshed_at), { locale: fr, addSuffix: true })}
              </span>
            ) : (
              <span className="ml-1 text-muted-foreground">· non chargés (offline ?)</span>
            )}
          </span>
          {pricing?.last_error && (
            <span className="text-destructive">Dernière erreur : {pricing.last_error}</span>
          )}
        </div>
        <Button
          size="sm"
          variant="outline"
          className="h-7 text-xs gap-1.5"
          onClick={() => refresh.mutate()}
          disabled={refresh.isPending}
          title={
            pricing?.last_refreshed_at
              ? `Dernière sync : ${format(new Date(pricing.last_refreshed_at), "d MMM yyyy · HH:mm:ss", { locale: fr })}`
              : "Charger les tarifs depuis openrouter.ai"
          }
        >
          {refresh.isPending ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <RefreshCw className="h-3 w-3" />
          )}
          Rafraîchir
        </Button>
      </div>

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
