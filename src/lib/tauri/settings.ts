import { invoke } from "@tauri-apps/api/core";
import type {
  AiKeyStatus,
  KeyValidationResult,
  ProviderInfo,
  AiProvider,
} from "@/types/settings.types";

export async function saveAiKey(
  provider: AiProvider,
  key: string
): Promise<KeyValidationResult> {
  return invoke<KeyValidationResult>("save_ai_key", { provider, key });
}

export async function testAiKey(provider: AiProvider): Promise<KeyValidationResult> {
  return invoke<KeyValidationResult>("test_ai_key", { provider });
}

export async function getAiKeyStatus(provider: AiProvider): Promise<AiKeyStatus> {
  return invoke<AiKeyStatus>("get_ai_key_status", { provider });
}

export async function deleteAiKey(provider: AiProvider): Promise<void> {
  return invoke<void>("delete_ai_key", { provider });
}

export async function setActiveProvider(
  provider: AiProvider,
  model: string
): Promise<void> {
  return invoke<void>("set_active_provider", { provider, model });
}

export async function getActiveProvider(): Promise<ProviderInfo> {
  return invoke<ProviderInfo>("get_active_provider");
}

// ── AI usage / cost tracker ──────────────────────────────────────────────

export interface UsageByModel {
  model: string;
  provider: string;
  calls: number;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  /** True when the cost was estimated from a fallback price rather than a known model entry. */
  price_estimated: boolean;
}

export interface UsageSummary {
  calls_30d: number;
  cost_usd_30d: number;
  cost_usd_month: number;
  by_model_30d: UsageByModel[];
}

export async function getAiUsageSummary(): Promise<UsageSummary> {
  return invoke<UsageSummary>("get_ai_usage_summary");
}

// ── OpenRouter live pricing ─────────────────────────────────────────────────

export interface PricingSnapshot {
  /** Map of model id → [input, output] price per million tokens, USD. */
  prices: Record<string, [number, number]>;
  /** RFC 3339 UTC timestamp of last successful fetch, or null if never run. */
  last_refreshed_at: string | null;
  /** Last error from a failed fetch, or null if last fetch succeeded. */
  last_error: string | null;
}

/** Read the current cached OpenRouter pricing snapshot without hitting the network. */
export async function getOpenRouterPricingSnapshot(): Promise<PricingSnapshot> {
  return invoke<PricingSnapshot>("get_openrouter_pricing_snapshot");
}

/** Force-refresh the OpenRouter pricing catalog. Returns the new snapshot. */
export async function refreshOpenRouterPricing(): Promise<PricingSnapshot> {
  return invoke<PricingSnapshot>("refresh_openrouter_pricing");
}
