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
  free?: boolean;
}

export const OPENROUTER_MODELS: ModelOption[] = [
  { value: "anthropic/claude-3-5-haiku", label: "Claude 3.5 Haiku (recommandé)" },
  { value: "anthropic/claude-3-5-sonnet", label: "Claude 3.5 Sonnet" },
  { value: "openai/gpt-4o-mini", label: "GPT-4o Mini" },
  { value: "openai/gpt-4o", label: "GPT-4o" },
  { value: "mistralai/mistral-small", label: "Mistral Small" },
  { value: "google/gemini-flash-1.5", label: "Gemini Flash 1.5" },
  { value: "deepseek/deepseek-chat", label: "DeepSeek Chat" },
  { value: "meta-llama/llama-3.2-3b-instruct:free", label: "Llama 3.2 3B (gratuit)", free: true },
  { value: "mistralai/mistral-7b-instruct:free", label: "Mistral 7B (gratuit)", free: true },
  { value: "qwen/qwen-2.5-7b-instruct:free", label: "Qwen 2.5 7B (gratuit)", free: true },
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
