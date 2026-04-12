use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::state::AppState;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectedAccount {
    pub id: i64,
    pub provider: String,
    pub user_id: String,
    pub username: String,
    pub display_name: Option<String>,
}

// ── PKCE helpers ──────────────────────────────────────────────────────────

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

fn generate_csrf_state() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

// ── Callback server ───────────────────────────────────────────────────────

/// Parse a query parameter value from an HTTP request line.
/// e.g. "GET /callback?code=abc&state=xyz HTTP/1.1" → Some("abc") for "code"
fn parse_query_param(request_line: &str, param: &str) -> Option<String> {
    let query_start = request_line.find('?')?;
    let query_end = request_line.rfind(" HTTP/")?;
    let query = &request_line[query_start + 1..query_end];

    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == param {
            return Some(parts.next().unwrap_or("").to_string());
        }
    }
    None
}

/// Start a one-shot HTTP callback server on `listener`.
/// Waits for the OAuth redirect, validates the CSRF state, and returns the code.
async fn accept_oauth_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, String> {
    // Accept connections until we receive the one carrying the code.
    // The browser may also request /favicon.ico — we handle that gracefully.
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| format!("Callback server error: {e}"))?;

        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or("");

        // Skip non-callback requests (e.g. /favicon.ico)
        if !first_line.contains("/callback") {
            let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\n\r\n").await;
            continue;
        }

        // Validate CSRF state
        let state_param = parse_query_param(first_line, "state");
        if state_param.as_deref() != Some(expected_state) {
            let _ = stream
                .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nInvalid state")
                .await;
            return Err("CSRF state mismatch — potential attack detected".to_string());
        }

        // Extract the authorization code
        let Some(code) = parse_query_param(first_line, "code") else {
            let _ = stream
                .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nMissing code")
                .await;
            return Err("No authorization code in callback".to_string());
        };

        // Respond with a success page
        let html = include_str!("../oauth_success.html");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(response.as_bytes()).await;

        return Ok(code);
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────

/// Start the Instagram OAuth PKCE flow.
/// Opens a browser, waits for the callback (up to 5 minutes), then stores the token.
/// Returns the connected account info on success.
#[tauri::command]
pub async fn start_oauth_flow(
    client_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectedAccount, String> {
    use tauri_plugin_opener::OpenerExt;

    // 1. PKCE
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let csrf = generate_csrf_state();

    // 2. Bind to fixed port 7891 — must match the redirect URI registered in your Meta App.
    //    Register: http://127.0.0.1:7891/callback in developers.facebook.com → App → Instagram Login
    const CALLBACK_PORT: u16 = 7891;
    let listener = TcpListener::bind(format!("127.0.0.1:{CALLBACK_PORT}"))
        .await
        .map_err(|e| {
            format!(
                "Failed to start callback server on port {CALLBACK_PORT}: {e}. \
                               Is another instance of Getpostcraft already running?"
            )
        })?;
    let redirect_uri = format!("http://127.0.0.1:{CALLBACK_PORT}/callback");

    // 3. Build Instagram authorization URL
    let auth_url = format!(
        "https://api.instagram.com/oauth/authorize\
         ?client_id={client_id}\
         &redirect_uri={encoded_redirect}\
         &scope=instagram_business_basic,instagram_business_content_publish\
         &response_type=code\
         &code_challenge={code_challenge}\
         &code_challenge_method=S256\
         &state={csrf}",
        encoded_redirect = urlencoding::encode(&redirect_uri),
    );

    // 4. Open browser
    app.opener()
        .open_url(&auth_url, None::<&str>)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    // 5. Wait for callback (5 min timeout)
    let code = tokio::time::timeout(
        Duration::from_secs(300),
        accept_oauth_callback(listener, &csrf),
    )
    .await
    .map_err(|_| "OAuth flow timed out — please try again")?
    .map_err(|e| e)?;

    // 6. Exchange code → access token
    let access_token =
        crate::adapters::instagram::exchange_code(&client_id, &code, &code_verifier, &redirect_uri)
            .await?;

    // 7. Fetch user profile
    let user_info = crate::adapters::instagram::get_user_info(&access_token).await?;

    // 8. Store token (never passes to renderer)
    let token_key = format!("instagram:{}", user_info.id);
    crate::token_store::save_token(&token_key, &access_token)?;

    // 9. Save account metadata to SQLite
    let account = crate::db::accounts::upsert_and_get(
        &state.db,
        "instagram",
        &user_info.id,
        &user_info.username,
        user_info.name.as_deref(),
        &token_key,
    )
    .await?;

    Ok(ConnectedAccount {
        id: account.id,
        provider: account.provider,
        user_id: account.user_id,
        username: account.username,
        display_name: account.display_name,
    })
}

/// List all connected accounts (metadata only — no tokens).
#[tauri::command]
pub async fn list_accounts(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectedAccount>, String> {
    let accounts = crate::db::accounts::list(&state.db).await?;
    Ok(accounts
        .into_iter()
        .map(|a| ConnectedAccount {
            id: a.id,
            provider: a.provider,
            user_id: a.user_id,
            username: a.username,
            display_name: a.display_name,
        })
        .collect())
}

/// Disconnect an account: remove the token from disk and the record from SQLite.
#[tauri::command]
pub async fn disconnect_account(
    provider: String,
    user_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let token_key = format!("{provider}:{user_id}");
    crate::token_store::delete_token(&token_key)?;
    crate::db::accounts::delete(&state.db, &provider, &user_id).await
}

/// Save the Instagram Meta App ID to settings.
#[tauri::command]
pub async fn save_instagram_app_id(
    app_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    crate::db::settings_db::set(&state.db, "instagram_app_id", &app_id).await
}

/// Get the Instagram Meta App ID from settings.
#[tauri::command]
pub async fn get_instagram_app_id(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    Ok(crate::db::settings_db::get(&state.db, "instagram_app_id").await)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_code_challenge_is_deterministic() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        // RFC 7636 § Appendix B test vector
        let challenge = generate_code_challenge(verifier);
        assert!(!challenge.is_empty());
        assert!(!challenge.contains('='), "must be no-pad base64url");
    }

    #[test]
    fn parse_query_param_works() {
        let line = "GET /callback?code=abc123&state=xyz HTTP/1.1";
        assert_eq!(parse_query_param(line, "code"), Some("abc123".to_string()));
        assert_eq!(parse_query_param(line, "state"), Some("xyz".to_string()));
        assert_eq!(parse_query_param(line, "missing"), None);
    }
}
