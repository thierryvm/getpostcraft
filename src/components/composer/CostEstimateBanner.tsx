import { useQuery } from "@tanstack/react-query";
import { Info } from "lucide-react";
import { getOpenRouterPricingSnapshot, getActiveProvider } from "@/lib/tauri/settings";
import { TauriRuntimeUnavailableError } from "@/lib/tauri/invoke";
import type { Network } from "@/types/composer.types";

/**
 * Per-network worst-case token estimate. Mirrors the upper bound of the
 * `max_tokens` budget in `sidecar/ai_client.py::generate_caption` so the
 * banner shows a number the user can actually be charged for. Real
 * generations almost always come in under this — the cap exists to avoid
 * silent truncation, not to be the average.
 *
 * Input tokens are dominated by the system prompt (network rules +
 * Product Truth) plus the brief itself. ~700 covers a generous
 * Product Truth and a 500-char brief on either network.
 */
const PER_NETWORK_TOKENS = {
  input: 700,
  output: { instagram: 600, linkedin: 1800, twitter: 280, tiktok: 600 } as Record<Network, number>,
};

interface CostEstimateBannerProps {
  /**
   * Networks the user has currently ticked. Empty array yields no banner —
   * we don't show "0 tokens estimated" on first paint, the form already has
   * the network selector immediately above which carries the message.
   */
  selectedNetworks: Network[];
}

/**
 * Compact banner that surfaces an upper-bound cost estimate for the
 * upcoming AI generation. Sits between the network selector and the
 * brief textarea so the user sees the bill before they hit Generate.
 *
 * Sources its prices from `pricing_cache` (the same OpenRouter snapshot
 * the AI usage panel uses) so it stays accurate when the user changes
 * model in Settings. When the cache is offline / empty we fall back to
 * a "coût indisponible" hint rather than a fake number.
 */
export function CostEstimateBanner({ selectedNetworks }: CostEstimateBannerProps) {
  const { data: provider } = useQuery({
    queryKey: ["active-provider"],
    queryFn: getActiveProvider,
    staleTime: 60_000,
  });

  const { data: pricing, error: pricingError } = useQuery({
    queryKey: ["openrouter-pricing-snapshot"],
    queryFn: getOpenRouterPricingSnapshot,
    staleTime: Number.POSITIVE_INFINITY,
  });

  if (selectedNetworks.length === 0) return null;

  // Dev-mode (no Tauri runtime) renders nothing — the DevModeBanner at
  // the app root already explains why IPC is unavailable, no need to
  // duplicate that signal in every panel.
  if (pricingError instanceof TauriRuntimeUnavailableError) return null;

  const modelKey = provider?.model ?? "";
  // Pricing snapshot is `Record<modelId, [inputUsdPerMTok, outputUsdPerMTok]>`.
  // Anthropic native models live on the static fallback table inside Rust
  // (they don't go through OpenRouter), so a missing entry isn't an
  // error — we just can't compute a number for them and show the
  // "indisponible" hint.
  const prices = pricing?.prices?.[modelKey];

  const inputUsdPerMTok = prices?.[0];
  const outputUsdPerMTok = prices?.[1];

  let estimateUsd: number | null = null;
  if (
    typeof inputUsdPerMTok === "number" &&
    typeof outputUsdPerMTok === "number"
  ) {
    let totalIn = 0;
    let totalOut = 0;
    for (const net of selectedNetworks) {
      totalIn += PER_NETWORK_TOKENS.input;
      totalOut += PER_NETWORK_TOKENS.output[net] ?? 600;
    }
    estimateUsd =
      (totalIn * inputUsdPerMTok) / 1_000_000 +
      (totalOut * outputUsdPerMTok) / 1_000_000;
  }

  const networkCount = selectedNetworks.length;
  const networkLabel = networkCount === 1 ? "1 réseau" : `${networkCount} réseaux`;

  return (
    <div
      className="flex items-start gap-2 rounded-md border border-border/60 bg-muted/30 px-3 py-2 text-xs text-muted-foreground"
      role="status"
      aria-live="polite"
    >
      <Info className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground/70" aria-hidden="true" />
      <div className="flex flex-col gap-0.5">
        {estimateUsd !== null ? (
          <>
            <span>
              Coût estimé{" "}
              <span className="font-mono text-foreground">
                ≈ {formatUsd(estimateUsd)}
              </span>{" "}
              pour <span className="text-foreground">{networkLabel}</span>
              {networkCount > 1 && (
                <span className="text-muted-foreground/70"> · prompts en parallèle</span>
              )}
            </span>
            <span className="text-[10px] text-muted-foreground/70">
              Borne haute (max_tokens) — le coût réel est généralement plus bas.
              Tarifs OpenRouter en direct.
            </span>
          </>
        ) : (
          <>
            <span>
              Coût estimé indisponible pour{" "}
              <span className="font-mono text-foreground">{modelKey || "modèle inconnu"}</span>.
            </span>
            <span className="text-[10px] text-muted-foreground/70">
              Vérifie ton dashboard fournisseur pour le tarif exact ; le compteur
              dans Paramètres → IA reste la source de vérité a posteriori.
            </span>
          </>
        )}
      </div>
    </div>
  );
}

/**
 * Format USD with up to 4 decimals when sub-cent precision matters
 * (multi-network on Sonnet 4.6 ≈ $0.003), capping at 2 decimals once
 * the figure exceeds $1. Same rule as the AI usage panel so the two
 * surfaces show the same shape.
 */
function formatUsd(n: number): string {
  if (n === 0) return "$0";
  if (n < 1) return `$${n.toFixed(4).replace(/\.?0+$/, "")}`;
  return `$${n.toFixed(2)}`;
}
