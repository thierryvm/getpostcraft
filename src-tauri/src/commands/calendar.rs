use crate::{db::history::PostRecord, state::AppState};

/// Fetch all posts whose scheduled_at (or created_at, for unscheduled posts) falls in [from, to].
/// Both dates are ISO-8601 strings (e.g. "2026-04-01T00:00:00Z" / "2026-04-30T23:59:59Z").
#[tauri::command]
pub async fn get_calendar_posts(
    state: tauri::State<'_, AppState>,
    from: String,
    to: String,
) -> Result<Vec<PostRecord>, String> {
    crate::db::history::list_in_range(&state.db, &from, &to).await
}

/// Assign a scheduled date to a post (ISO-8601 string).
#[tauri::command]
pub async fn schedule_post(
    state: tauri::State<'_, AppState>,
    post_id: i64,
    scheduled_at: String,
) -> Result<(), String> {
    crate::db::history::set_scheduled_at(&state.db, post_id, Some(&scheduled_at)).await
}

/// Remove the scheduled date from a post (revert to draft).
#[tauri::command]
pub async fn unschedule_post(
    state: tauri::State<'_, AppState>,
    post_id: i64,
) -> Result<(), String> {
    crate::db::history::set_scheduled_at(&state.db, post_id, None).await
}

/// Delete a post permanently.
#[tauri::command]
pub async fn delete_post(state: tauri::State<'_, AppState>, post_id: i64) -> Result<(), String> {
    crate::db::history::delete_post(&state.db, post_id).await
}

/// Update caption and hashtags of a draft post.
#[tauri::command]
pub async fn update_post_draft(
    state: tauri::State<'_, AppState>,
    post_id: i64,
    caption: String,
    hashtags: Vec<String>,
) -> Result<(), String> {
    crate::db::history::update_draft_content(&state.db, post_id, &caption, &hashtags).await
}
