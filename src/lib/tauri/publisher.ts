import { invoke } from "@tauri-apps/api/core";

export interface PublishResult {
  post_id: number;
  ig_media_id: string;
  published_at: string;
}

/** Publish a draft post to Instagram. The post must have an image attached. */
export function publishPost(postId: number): Promise<PublishResult> {
  return invoke<PublishResult>("publish_post", { postId });
}

export function saveImgbbKey(apiKey: string): Promise<void> {
  return invoke<void>("save_imgbb_key", { apiKey });
}

export function getImgbbKeyStatus(): Promise<boolean> {
  return invoke<boolean>("get_imgbb_key_status");
}

/** Attach a base64 data URL (or file path) to a draft post so publish_post can find it. */
export function updateDraftImage(postId: number, imagePath: string): Promise<void> {
  return invoke<void>("update_draft_image", { postId, imagePath });
}
