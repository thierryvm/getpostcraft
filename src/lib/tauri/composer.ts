import { invoke } from "@tauri-apps/api/core";
import type { GeneratedContent, Network, PostRecord } from "@/types/composer.types";

export async function generateContent(
  brief: string,
  network: Network
): Promise<GeneratedContent> {
  return invoke<GeneratedContent>("generate_content", { brief, network });
}

export async function saveDraft(
  network: Network,
  caption: string,
  hashtags: string[]
): Promise<number> {
  return invoke<number>("save_draft", { network, caption, hashtags });
}

export async function getPostHistory(limit?: number): Promise<PostRecord[]> {
  return invoke<PostRecord[]>("get_post_history", { limit });
}
