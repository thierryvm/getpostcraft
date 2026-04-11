import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Render caption + hashtags to a 1080×1080 PNG via the Python sidecar.
 * Returns a displayable URL using the Tauri asset protocol.
 */
export async function renderPostImage(
  caption: string,
  hashtags: string[]
): Promise<string> {
  const filePath = await invoke<string>("render_post_image", {
    caption,
    hashtags,
  });
  return convertFileSrc(filePath);
}
