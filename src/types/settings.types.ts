export type AiProvider = "openrouter" | "anthropic" | "ollama";

export interface ProviderMeta {
  label: string;
  keyPrefix: string | null; // null = no key needed
}

export const PROVIDER_META: Record<AiProvider, ProviderMeta> = {
  openrouter: { label: "OpenRouter (recommandé)", keyPrefix: "sk-or-" },
  anthropic: { label: "Anthropic direct", keyPrefix: "sk-ant-" },
  ollama: { label: "Ollama local", keyPrefix: null },
};

export interface ModelOption {
  value: string;
  label: string;
  /** Cost per 1M input tokens in USD. undefined = free or unknown. */
  inputPricePer1M?: number;
  /** Cost per 1M output tokens in USD. undefined = free or unknown. */
  outputPricePer1M?: number;
  free?: boolean;
  /** Free-tier endpoints come and go on OpenRouter — may return 404 at any time. */
  unstable?: boolean;
  /**
   * Model does not reliably follow JSON-only output instructions.
   * May produce preamble text or suffix that breaks the sidecar parser.
   * Validated via sidecar/tests/test_ai_client.py::TestModelOutputPatterns.
   */
  jsonUnreliable?: boolean;
}

// OpenRouter slugs use **dots** in version numbers (e.g. `claude-sonnet-4.6`),
// NOT dashes. The Anthropic-native API uses dashes + date-stamped snapshots
// (e.g. `claude-haiku-4-5-20251001`) — those are not accepted by OpenRouter.
// Source of truth: https://openrouter.ai/api/v1/models (verified 2026-05-07).
export const OPENROUTER_MODELS: ModelOption[] = [
  // ── Anthropic Claude (latest aliases — never break on version bumps) ────
  { value: "anthropic/claude-haiku-latest",   label: "Claude Haiku (recommandé)", inputPricePer1M: 1.00,  outputPricePer1M: 5.00  },
  { value: "anthropic/claude-sonnet-4.6",     label: "Claude Sonnet 4.6",         inputPricePer1M: 3.00,  outputPricePer1M: 15.00 },
  { value: "anthropic/claude-sonnet-latest",  label: "Claude Sonnet (latest)",    inputPricePer1M: 3.00,  outputPricePer1M: 15.00 },
  { value: "anthropic/claude-opus-4.7",       label: "Claude Opus 4.7",           inputPricePer1M: 5.00,  outputPricePer1M: 25.00 },
  { value: "anthropic/claude-opus-4.6-fast",  label: "Claude Opus 4.6 Fast",      inputPricePer1M: 30.00, outputPricePer1M: 150.00 },
  { value: "anthropic/claude-opus-latest",    label: "Claude Opus (latest)",      inputPricePer1M: 5.00,  outputPricePer1M: 25.00 },
  // ── Other providers (slugs unchanged) ─────────────────────────────────────
  { value: "openai/gpt-4o-mini",                      label: "GPT-4o Mini",                   inputPricePer1M: 0.15,  outputPricePer1M: 0.60  },
  { value: "openai/gpt-4o",                           label: "GPT-4o",                        inputPricePer1M: 2.50,  outputPricePer1M: 10.00 },
  { value: "deepseek/deepseek-chat",                  label: "DeepSeek V3",                   inputPricePer1M: 0.32,  outputPricePer1M: 0.89  },
  { value: "deepseek/deepseek-chat-v3.1",             label: "DeepSeek V3.1",                 inputPricePer1M: 0.15,  outputPricePer1M: 0.75  },
  { value: "google/gemini-2.0-flash-001",             label: "Gemini 2.0 Flash",              inputPricePer1M: 0.10,  outputPricePer1M: 0.40  },
  // ── JSON peu fiable — peut échouer selon le brief ────────────────────────
  { value: "mistralai/mistral-small-3.1-24b-instruct", label: "Mistral Small 3.1 ⚠️",          inputPricePer1M: 0.10,  outputPricePer1M: 0.30,  jsonUnreliable: true },
  // ── Gratuits — instables (endpoint peut retourner 404 à tout moment) ─────
  { value: "meta-llama/llama-3.3-70b-instruct:free",  label: "Llama 3.3 70B (gratuit)",       free: true, unstable: true },
  { value: "qwen/qwen-2.5-72b-instruct:free",         label: "Qwen 2.5 72B (gratuit)",        free: true, unstable: true },
];

export const PROVIDER_DEFAULT_MODELS: Record<AiProvider, string> = {
  // Sonnet 4.6 par défaut : meilleur ratio qualité/coût pour la création de
  // contenu marketing (~$0.30/mois à 30 posts), bien plus créatif que Haiku.
  openrouter: "anthropic/claude-sonnet-4.6",
  // L'API Anthropic native garde le format snapshot daté.
  anthropic: "claude-haiku-4-5-20251001",
  ollama: "llama3.2",
};

export interface AiKeyStatus {
  configured: boolean;
  masked: string | null;
}

export interface KeyValidationResult {
  valid: boolean;
  error?: string;
}

export interface ProviderInfo {
  provider: AiProvider;
  model: string;
}
