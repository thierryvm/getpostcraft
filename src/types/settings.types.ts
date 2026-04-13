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
}

export const OPENROUTER_MODELS: ModelOption[] = [
  { value: "anthropic/claude-3-5-haiku",             label: "Claude 3.5 Haiku (recommandé)", inputPricePer1M: 0.80,  outputPricePer1M: 4.00  },
  { value: "anthropic/claude-3-5-sonnet",            label: "Claude 3.5 Sonnet",             inputPricePer1M: 3.00,  outputPricePer1M: 15.00 },
  { value: "openai/gpt-4o-mini",                     label: "GPT-4o Mini",                   inputPricePer1M: 0.15,  outputPricePer1M: 0.60  },
  { value: "openai/gpt-4o",                          label: "GPT-4o",                        inputPricePer1M: 2.50,  outputPricePer1M: 10.00 },
  { value: "mistralai/mistral-small",                label: "Mistral Small",                 inputPricePer1M: 0.10,  outputPricePer1M: 0.30  },
  { value: "google/gemini-flash-1.5",                label: "Gemini Flash 1.5",              inputPricePer1M: 0.075, outputPricePer1M: 0.30  },
  { value: "deepseek/deepseek-chat",                 label: "DeepSeek V3",                   inputPricePer1M: 0.27,  outputPricePer1M: 1.10  },
  { value: "deepseek/deepseek-chat-v3-5:free",      label: "DeepSeek V3.5 (gratuit)",       free: true, unstable: true },
  { value: "google/gemini-2.0-flash-exp:free",        label: "Gemini 2.0 Flash (gratuit)",    free: true, unstable: true },
  { value: "meta-llama/llama-3.3-70b-instruct:free", label: "Llama 3.3 70B (gratuit)",       free: true, unstable: true },
  { value: "qwen/qwen-2.5-72b-instruct:free",        label: "Qwen 2.5 72B (gratuit)",        free: true, unstable: true },
];

export const PROVIDER_DEFAULT_MODELS: Record<AiProvider, string> = {
  openrouter: "anthropic/claude-3-5-haiku",
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
