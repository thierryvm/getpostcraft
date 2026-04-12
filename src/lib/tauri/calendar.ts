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
