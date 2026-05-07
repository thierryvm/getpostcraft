import { invoke } from "@tauri-apps/api/core";
import type { GeneratedContent, Network, PostRecord } from "@/types/composer.types";

/** Scrape a URL and return extracted text ready to use as a brief. */
export async function scrapeUrlForBrief(url: string): Promise<string> {
  return invoke<string>("scrape_url_for_brief", { url });
}

/** Fire-and-forget: warm up the Python sidecar when Composer mounts. */
export function warmupSidecar(): void {
  invoke("warmup_sidecar").catch(() => {
    // Intentionally silent — warmup failure is non-critical
  });
}

export async function generateContent(
  brief: string,
  network: Network,
  accountId: number | null
): Promise<GeneratedContent> {
  return invoke<GeneratedContent>("generate_content", {
    brief,
    network,
    accountId,
  });
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

export interface CaptionVariant {
  tone: "educational" | "casual" | "punchy";
  caption: string;
  hashtags: string[];
}

export async function generateVariants(
  brief: string,
  network: string,
  accountId: number | null
): Promise<CaptionVariant[]> {
  return invoke<CaptionVariant[]>("generate_variants", {
    brief,
    network,
    accountId,
  });
}

export interface CarouselSlide {
  index: number;
  total: number;
  emoji: string;
  title: string;
  body: string;
}

export async function generateCarousel(
  brief: string,
  network: string,
  slideCount: number,
  accountId: number | null
): Promise<CarouselSlide[]> {
  return invoke<CarouselSlide[]>("generate_carousel", {
    brief,
    network,
    slideCount,
    accountId,
  });
}
