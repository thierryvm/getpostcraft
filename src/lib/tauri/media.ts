import { invoke } from "@tauri-apps/api/core";

/** Render caption + hashtags → 1080×1080 PNG data URL. */
export async function renderPostImage(
  caption: string,
  hashtags: string[]
): Promise<string> {
  return invoke<string>("render_post_image", { caption, hashtags });
}

/** Render a code snippet card → 1080×1080 PNG data URL. */
export async function renderCodeImage(
  code: string,
  language: string,
  filename?: string
): Promise<string> {
  return invoke<string>("render_code_image", { code, language, filename: filename ?? null });
}

/** Render a terminal mockup → 1080×1080 PNG data URL. */
export async function renderTerminalImage(
  command: string,
  output?: string
): Promise<string> {
  return invoke<string>("render_terminal_image", { command, output: output ?? null });
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
