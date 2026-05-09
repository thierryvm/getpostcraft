import { invoke } from "@tauri-apps/api/core";

/** Optional account-specific branding for the rendered image. */
export interface BrandOptions {
  handle?: string | null;
  brandColor?: string | null;
}

/** Render caption + hashtags → PNG data URL at the given dimensions (default 1080×1080). */
export async function renderPostImage(
  caption: string,
  hashtags: string[],
  width = 1080,
  height = 1080,
  brand: BrandOptions = {},
): Promise<string> {
  return invoke<string>("render_post_image", {
    caption,
    hashtags,
    width,
    height,
    handle: brand.handle ?? null,
    brandColor: brand.brandColor ?? null,
  });
}

/** Render a code snippet card → PNG data URL at the given dimensions (default 1080×1080). */
export async function renderCodeImage(
  code: string,
  language: string,
  filename?: string,
  width = 1080,
  height = 1080,
  brand: BrandOptions = {},
): Promise<string> {
  return invoke<string>("render_code_image", {
    code,
    language,
    filename: filename ?? null,
    width,
    height,
    handle: brand.handle ?? null,
    brandColor: brand.brandColor ?? null,
  });
}

/** Render a terminal mockup → PNG data URL at the given dimensions (default 1080×1080). */
export async function renderTerminalImage(
  command: string,
  output?: string,
  width = 1080,
  height = 1080,
  brand: BrandOptions = {},
): Promise<string> {
  return invoke<string>("render_terminal_image", {
    command,
    output: output ?? null,
    width,
    height,
    handle: brand.handle ?? null,
    brandColor: brand.brandColor ?? null,
  });
}

/** Render carousel slides → array of base64 PNG data URLs (same order as input).
 *  Defaults to the Instagram 4:5 portrait (1080×1350); pass `width`/`height`
 *  to follow whatever format the user picks in the Composer. */
export async function renderCarouselSlides(
  slides: import("@/lib/tauri/composer").CarouselSlide[],
  brand: BrandOptions = {},
  width = 1080,
  height = 1350,
): Promise<string[]> {
  return invoke<string[]>("render_carousel_slides", {
    slides,
    handle: brand.handle ?? null,
    brandColor: brand.brandColor ?? null,
    width,
    height,
  });
}

/** Pack rendered carousel images into a ZIP in Downloads. Returns the ZIP file path. */
export async function exportCarouselZip(images: string[]): Promise<string> {
  return invoke<string>("export_carousel_zip", { images });
}
