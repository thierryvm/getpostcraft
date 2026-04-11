import { invoke } from "@tauri-apps/api/core";
import type { GeneratedContent, Network } from "@/types/composer.types";

export async function generateContent(
  brief: string,
  network: Network
): Promise<GeneratedContent> {
  return invoke<GeneratedContent>("generate_content", { brief, network });
}
