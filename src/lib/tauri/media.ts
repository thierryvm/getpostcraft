import { invoke } from "@tauri-apps/api/core";

/** Render caption + hashtags → PNG data URL at the given dimensions (default 1080×1080). */
export async function renderPostImage(
  caption: string,
  hashtags: string[],
  width = 1080,
  height = 1080,
): Promise<string> {
  return invoke<string>("render_post_image", { caption, hashtags, width, height });
}

/** Render a code snippet card → PNG data URL at the given dimensions (default 1080×1080). */
export async function renderCodeImage(
  code: string,
  language: string,
  filename?: string,
  width = 1080,
  height = 1080,
): Promise<string> {
  return invoke<string>("render_code_image", { code, language, filename: filename ?? null, width, height });
}

/** Render a terminal mockup → PNG data URL at the given dimensions (default 1080×1080). */
export async function renderTerminalImage(
  command: string,
  output?: string,
  width = 1080,
  height = 1080,
): Promise<string> {
  return invoke<string>("render_terminal_image", { command, output: output ?? null, width, height });
}

/** Render carousel slides → array of base64 PNG data URLs (same order as input). */
export async function renderCarouselSlides(
  slides: import("@/lib/tauri/composer").CarouselSlide[]
): Promise<string[]> {
  return invoke<string[]>("render_carousel_slides", { slides });
}

/** Pack rendered carousel images into a ZIP in Downloads. Returns the ZIP file path. */
export async function exportCarouselZip(images: string[]): Promise<string> {
  return invoke<string>("export_carousel_zip", { images });
}
