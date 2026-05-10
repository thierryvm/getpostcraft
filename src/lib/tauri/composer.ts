import { invoke } from "./invoke";
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
  hashtags: string[],
  accountId: number | null = null,
): Promise<number> {
  return invoke<number>("save_draft", { network, caption, hashtags, accountId });
}

export async function getPostHistory(limit?: number): Promise<PostRecord[]> {
  return invoke<PostRecord[]>("get_post_history", { limit });
}

/** Fetch a single post by id — used to reload a draft into the composer. */
export async function getPostById(postId: number): Promise<PostRecord> {
  return invoke<PostRecord>("get_post_by_id", { postId });
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

// ── Multi-network composer (v0.3.9) ──────────────────────────────────────────

/** One slot in the multi-network Composer. account_id is per-network so the
 *  user can target a specific connected account on networks where they have
 *  several (only LinkedIn supports that today, but the type is forward-
 *  compatible). */
export interface GroupNetworkRequest {
  network: Network;
  account_id: number | null;
}

/** Per-network outcome of a group generation. On success, post_id +
 *  caption + hashtags are populated and error_message is null; on failure
 *  the inverse. The Composer renders one tab per member regardless of
 *  status so the user can retry just the failures. */
export interface GroupMemberResult {
  network: Network;
  status: "ok" | "error";
  post_id: number | null;
  caption: string | null;
  hashtags: string[] | null;
  error_message: string | null;
}

/** Result of a multi-network generation. group_id is null when EVERY
 *  member failed — the backend skips creating a parent in that case so
 *  the dashboard never shows phantom empty groups. */
export interface GroupGenerationResult {
  group_id: number | null;
  members: GroupMemberResult[];
}

/** Generate captions for N networks in parallel and persist them as a
 *  single post_groups parent + N sibling drafts. Best-effort: a single
 *  failing network does NOT abort the whole flow — the user gets the
 *  successes immediately. The brief must be ≥ 10 chars and `networks`
 *  must hold 1-3 distinct entries (V1 ceiling). */
export async function generateAndSaveGroup(
  brief: string,
  networks: GroupNetworkRequest[],
): Promise<GroupGenerationResult> {
  return invoke<GroupGenerationResult>("generate_and_save_group", {
    brief,
    networks,
  });
}

/** Allowed values for `CarouselSlide.role`. Mirrors the Rust whitelist
 *  in `commands::media::role_meta_for`. Anything else is normalised to
 *  `null` by the sidecar before reaching the frontend. */
export type CarouselSlideRole =
  | "hero"
  | "problem"
  | "approach"
  | "tech"
  | "change"
  | "moment"
  | "cta";

export interface CarouselSlide {
  index: number;
  total: number;
  emoji: string;
  title: string;
  body: string;
  /** Section role suggested by the AI; drives badge color and label in
   *  the rendered image. `null` falls back to an index-derived label. */
  role: CarouselSlideRole | null;
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

/**
 * Render a URL with Playwright and synthesize a ProductTruth ready to paste
 * into Settings → Comptes. Two-step pipeline (scrape + AI) — can take ~15-30 s
 * on cold runs because Playwright launches Chromium and the AI synthesis budget
 * is generous (1200 tokens). Caller should show a spinner.
 */
export async function synthesizeProductTruthFromUrl(
  url: string,
  handle: string,
): Promise<string> {
  return invoke<string>("synthesize_product_truth_from_url", { url, handle });
}

/** Structured visual brand profile extracted from a website screenshot. */
export interface VisualProfile {
  colors: string[];
  typography: { family: string; weight: string; character: string };
  mood: string[];
  layout: string;
}

export interface WebsiteAnalysis {
  product_truth: string;
  visual_profile: VisualProfile;
}

/**
 * Single-call URL analyzer that does both: scrape + AI synthesis (text) +
 * Vision extraction (visual profile). One Playwright launch, ~25-40 s end
 * to end. If `accountId` is provided the visual_profile is persisted on
 * the account so future post generations can reuse it.
 */
export async function analyzeUrlVisual(
  url: string,
  handle: string,
  accountId: number | null,
): Promise<WebsiteAnalysis> {
  return invoke<WebsiteAnalysis>("analyze_url_visual", { url, handle, accountId });
}
