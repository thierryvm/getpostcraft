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
