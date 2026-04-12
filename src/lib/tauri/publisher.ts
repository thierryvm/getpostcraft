import { invoke } from "@tauri-apps/api/core";

export interface PublishResult {
  post_id: number;
  /** Platform-specific media ID or post URN (Instagram media ID, LinkedIn ugcPost URN, etc.) */
  media_id: string;
  published_at: string;
}

/** Publish a draft post to Instagram. The post must have an image attached. */
export function publishPost(postId: number): Promise<PublishResult> {
  return invoke<PublishResult>("publish_post", { postId });
}

/** Publish a draft post to LinkedIn (text-only or with image). Max 5 hashtags. */
export function publishLinkedinPost(postId: number): Promise<PublishResult> {
  return invoke<PublishResult>("publish_linkedin_post", { postId });
}

export function saveImgbbKey(apiKey: string): Promise<void> {
  return invoke<void>("save_imgbb_key", { apiKey });
}

export function getImgbbKeyStatus(): Promise<boolean> {
  return invoke<boolean>("get_imgbb_key_status");
}

/** Attach a base64 data URL (or file path) to a draft post so publish commands can find it. */
export function updateDraftImage(postId: number, imagePath: string): Promise<void> {
  return invoke<void>("update_draft_image", { postId, imagePath });
}
