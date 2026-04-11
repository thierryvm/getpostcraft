import { invoke } from "@tauri-apps/api/core";

/**
 * Render caption + hashtags to a 1080×1080 PNG via the Python sidecar.
 * Returns a data:image/png;base64,... URL ready for use in an <img> src.
 */
export async function renderPostImage(
  caption: string,
  hashtags: string[]
): Promise<string> {
  return invoke<string>("render_post_image", { caption, hashtags });
}
