//! Tauri commands for the Settings → Security gate.
//!
//! Thin wrappers over `crate::security_admin` that map module types to
//! IPC-friendly shapes (snake_case JSON via serde). No business logic here.

use crate::security_admin::{
    check_session, end_session, is_password_set, setup_password, verify_password, VerifyOutcome,
};
use crate::state::AppState;
use serde::Serialize;

/// Renderer-facing shape for [`VerifyOutcome`]. Tagged union (`kind`)
/// keeps the JSON unambiguous without leaning on serde defaults that
/// strip variant data.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerifyResponse {
    Ok { token: String },
    LockedOut { wait_seconds: u64 },
    NoPasswordSet,
    Wrong,
}

impl From<VerifyOutcome> for VerifyResponse {
    fn from(o: VerifyOutcome) -> Self {
        match o {
            VerifyOutcome::Ok { token_b64 } => VerifyResponse::Ok { token: token_b64 },
            VerifyOutcome::LockedOut { wait } => VerifyResponse::LockedOut {
                wait_seconds: wait.as_secs(),
            },
            VerifyOutcome::NoPasswordSet => VerifyResponse::NoPasswordSet,
            VerifyOutcome::Wrong => VerifyResponse::Wrong,
        }
    }
}

/// Audit-log row exposed to the renderer. Only fields the UI needs.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AttemptRow {
    pub id: i64,
    pub attempted_at: String,
    pub success: i64,
    pub note: Option<String>,
}

#[tauri::command]
pub fn is_security_password_set() -> bool {
    is_password_set()
}

#[tauri::command]
pub async fn setup_security_password(plain: String) -> Result<(), String> {
    setup_password(&plain)
}

#[tauri::command]
pub async fn verify_security_password(
    state: tauri::State<'_, AppState>,
    plain: String,
) -> Result<VerifyResponse, String> {
    let outcome = verify_password(&plain, &state.security_admin, Some(&state.db)).await;
    Ok(outcome.into())
}

#[tauri::command]
pub fn check_security_session(state: tauri::State<'_, AppState>, token: String) -> bool {
    check_session(&state.security_admin, &token)
}

#[tauri::command]
pub fn end_security_session(state: tauri::State<'_, AppState>) {
    end_session(&state.security_admin);
}

#[tauri::command]
pub async fn list_recent_security_attempts(
    state: tauri::State<'_, AppState>,
    limit: i64,
) -> Result<Vec<AttemptRow>, String> {
    let capped = limit.clamp(1, 200);
    sqlx::query_as::<_, AttemptRow>(
        "SELECT id, attempted_at, success, note \
         FROM security_audit_attempts \
         ORDER BY attempted_at DESC LIMIT ?",
    )
    .bind(capped)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Cannot read audit attempts: {e}"))
}
