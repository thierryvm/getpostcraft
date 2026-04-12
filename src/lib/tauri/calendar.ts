import { invoke } from "@tauri-apps/api/core";
import type { PostRecord } from "@/types/composer.types";

/** Fetch posts in range [from, to] (ISO-8601 datetime strings). */
export async function getCalendarPosts(
  from: string,
  to: string
): Promise<PostRecord[]> {
  return invoke<PostRecord[]>("get_calendar_posts", { from, to });
}

/** Assign a scheduled date (ISO-8601) to a post. */
export async function schedulePost(
  postId: number,
  scheduledAt: string
): Promise<void> {
  return invoke("schedule_post", { post_id: postId, scheduled_at: scheduledAt });
}

/** Remove the scheduled date from a post. */
export async function unschedulePost(postId: number): Promise<void> {
  return invoke("unschedule_post", { post_id: postId });
}

/** Delete a post permanently. */
export async function deletePost(postId: number): Promise<void> {
  return invoke("delete_post", { post_id: postId });
}

/** Update caption and hashtags of a draft post. */
export async function updatePostDraft(
  postId: number,
  caption: string,
  hashtags: string[]
): Promise<void> {
  return invoke("update_post_draft", { post_id: postId, caption, hashtags });
}
