use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::state::AppState;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectedAccount {
    pub id: i64,
    pub provider: String,
    pub user_id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub product_truth: Option<String>,
    pub brand_color: Option<String>,
    pub accent_color: Option<String>,
    pub visual_profile: Option<String>,
}

impl From<crate::db::accounts::Account> for ConnectedAccount {
    fn from(a: crate::db::accounts::Account) -> Self {
        Self {
            id: a.id,
            provider: a.provider,
            user_id: a.user_id,
            username: a.username,
            display_name: a.display_name,
            product_truth: a.product_truth,
            brand_color: a.brand_color,
            accent_color: a.accent_color,
            visual_profile: a.visual_profile,
        }
    }
}

// ── PKCE helpers ──────────────────────────────────────────────────────────

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

fn generate_csrf_state() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

// ── TLS helper ────────────────────────────────────────────────────────────

/// Build a self-signed TLS acceptor for localhost.
/// The certificate is generated fresh each OAuth flow — it only needs to live
/// long enough for the browser to redirect back (a few seconds).
fn build_tls_acceptor() -> Result<TlsAcceptor, String> {
    let certified_key = generate_simple_self_signed(vec!["localhost".to_string()])
        .map_err(|e| format!("Failed to generate TLS cert: {e}"))?;

    let cert_der = certified_key.cert.der().to_vec();
    let key_der = certified_key.key_pair.serialize_der();

    let cert_chain = vec![CertificateDer::from(cert_der)];
    let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der));

    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .map_err(|e| format!("TLS config error: {e}"))?;

    Ok(TlsAcceptor::from(Arc::new(server_config)))
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

/// Start a one-shot HTTPS callback server on `listener`.
/// Waits for the OAuth redirect, validates the CSRF state, and returns the code.
/// The browser will show a self-signed cert warning on first use — this is expected.
async fn accept_oauth_callback(
    listener: TcpListener,
    acceptor: TlsAcceptor,
    expected_state: &str,
) -> Result<String, String> {
    loop {
        let (tcp_stream, _) = listener
            .accept()
            .await
            .map_err(|e| format!("Callback server error: {e}"))?;

        // TLS handshake — skip connections that fail (e.g. browser pre-flight TCP probes)
        let mut tls_stream = match acceptor.accept(tcp_stream).await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut buf = vec![0u8; 8192];
        let n = tls_stream.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or("");

        // Skip non-callback requests (e.g. /favicon.ico)
        if !first_line.contains("/callback") {
            let _ = tls_stream
                .write_all(b"HTTP/1.1 204 No Content\r\n\r\n")
                .await;
            continue;
        }

        // Validate CSRF state
        let state_param = parse_query_param(first_line, "state");
        if state_param.as_deref() != Some(expected_state) {
            let _ = tls_stream
                .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nInvalid state")
                .await;
            return Err("CSRF state mismatch — potential attack detected".to_string());
        }

        // Extract the authorization code
        let Some(code) = parse_query_param(first_line, "code") else {
            let _ = tls_stream
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
        let _ = tls_stream.write_all(response.as_bytes()).await;

        return Ok(code);
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────

/// Start the Instagram OAuth PKCE flow.
/// Opens a browser to Instagram, waits for the HTTPS callback (up to 5 minutes),
/// then exchanges the code and stores the token securely.
///
/// Prerequisites:
///   - instagram_app_id configured in settings (Meta Instagram App ID)
///   - instagram_client_secret stored via save_instagram_client_secret
///   - https://localhost:7891/callback registered in Meta App → Instagram → OAuth redirect URIs
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

    // 2. Retrieve client_secret (required by Meta even with PKCE)
    let client_secret = crate::ai_keys::get_key("instagram_client_secret").map_err(|_| {
        "Instagram client secret not configured. Add it in Settings → Comptes.".to_string()
    })?;

    // 3. Bind to fixed port 7891 — must match the redirect URI in your Meta App.
    //    Register: https://localhost:7891/callback in developers.facebook.com
    //    → App → Instagram Login product → Settings → Valid OAuth redirect URIs
    const CALLBACK_PORT: u16 = 7891;
    let listener = TcpListener::bind(format!("127.0.0.1:{CALLBACK_PORT}"))
        .await
        .map_err(|e| {
            format!(
                "Failed to start callback server on port {CALLBACK_PORT}: {e}. \
                 Is another instance of Getpostcraft already running?"
            )
        })?;
    let redirect_uri = format!("https://localhost:{CALLBACK_PORT}/callback");

    // 4. Build TLS acceptor (self-signed cert for localhost)
    let acceptor = build_tls_acceptor()?;

    // 5. Build Instagram authorization URL
    let auth_url = format!(
        "https://www.instagram.com/oauth/authorize\
         ?client_id={client_id}\
         &redirect_uri={encoded_redirect}\
         &scope=instagram_business_basic,instagram_business_content_publish\
         &response_type=code\
         &code_challenge={code_challenge}\
         &code_challenge_method=S256\
         &state={csrf}",
        encoded_redirect = urlencoding::encode(&redirect_uri),
    );

    // 6. Open browser
    app.opener()
        .open_url(&auth_url, None::<&str>)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    // 7. Wait for HTTPS callback (5 min timeout)
    let code = tokio::time::timeout(
        Duration::from_secs(300),
        accept_oauth_callback(listener, acceptor, &csrf),
    )
    .await
    .map_err(|_| "OAuth flow timed out — please try again")??;

    // 8. Exchange code → short-lived token, then immediately upgrade to long-lived (~60 days)
    let short_lived = crate::adapters::instagram::exchange_code(
        &client_id,
        &client_secret,
        &code,
        &code_verifier,
        &redirect_uri,
    )
    .await?;

    let access_token = match crate::adapters::instagram::exchange_for_long_lived_token(
        &short_lived,
        &client_secret,
    )
    .await
    {
        Ok(token) => {
            log::info!("Instagram: long-lived token obtained successfully");
            token
        }
        Err(e) => {
            log::warn!("Instagram: long-lived token exchange failed, falling back to short-lived token: {e}");
            short_lived
        }
    };

    // 9. Fetch user profile
    let user_info = crate::adapters::instagram::get_user_info(&access_token).await?;

    // 10. Store token (never passes to renderer)
    let token_key = format!("instagram:{}", user_info.id);
    crate::token_store::save_token(&token_key, &access_token)?;

    // 11. Save account metadata to SQLite
    let account = crate::db::accounts::upsert_and_get(
        &state.db,
        "instagram",
        &user_info.id,
        &user_info.username,
        user_info.name.as_deref(),
        &token_key,
    )
    .await?;

    Ok(account.into())
}

/// List all connected accounts (metadata only — no tokens).
#[tauri::command]
pub async fn list_accounts(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectedAccount>, String> {
    let accounts = crate::db::accounts::list(&state.db).await?;
    Ok(accounts.into_iter().map(Into::into).collect())
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

/// Save or clear the product truth text for a connected account.
/// Passing an empty string clears the field (sets to NULL).
#[tauri::command]
pub async fn update_account_product_truth(
    state: tauri::State<'_, AppState>,
    account_id: i64,
    product_truth: String,
) -> Result<(), String> {
    let value = if product_truth.trim().is_empty() {
        None
    } else {
        Some(product_truth.trim())
    };
    crate::db::accounts::update_product_truth(&state.db, account_id, value).await
}

/// Strict hex color whitelist: empty → None (clear), or '#' followed by exactly
/// 3 or 6 ASCII hex digits. Rejects malformed values (e.g. `#g!;{}`, `#abc"}>`)
/// so the result can be safely interpolated into `<style>` blocks without
/// HTML/CSS escaping. Pulled out of the Tauri command so it can be unit-tested.
fn parse_hex_color(value: &str) -> Result<Option<&str>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let valid = trimmed.starts_with('#')
        && (trimmed.len() == 4 || trimmed.len() == 7)
        && trimmed[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(format!(
            "Couleur invalide « {trimmed} » — utilise un format hex (#rgb ou #rrggbb)."
        ));
    }
    Ok(Some(trimmed))
}

/// Save or clear branding colors (brand + accent) for an account.
/// Empty strings are treated as "clear" (set to NULL — falls back to app defaults).
#[tauri::command]
pub async fn update_account_branding(
    state: tauri::State<'_, AppState>,
    account_id: i64,
    brand_color: String,
    accent_color: String,
) -> Result<(), String> {
    let brand = parse_hex_color(&brand_color)?;
    let accent = parse_hex_color(&accent_color)?;
    crate::db::accounts::update_branding(&state.db, account_id, brand, accent).await
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

/// Save the Instagram app client_secret.
/// SECURITY: stored in api_keys.json (user data dir), never crosses IPC back to renderer.
#[tauri::command]
pub fn save_instagram_client_secret(secret: String) -> Result<(), String> {
    crate::ai_keys::save_key("instagram_client_secret", &secret)
}

/// Check if the Instagram client_secret is configured.
#[tauri::command]
pub fn get_instagram_client_secret_status() -> bool {
    crate::ai_keys::has_key("instagram_client_secret")
}

// ── LinkedIn OAuth ────────────────────────────────────────────────────────

/// Start the LinkedIn OAuth PKCE flow.
/// Opens a browser to LinkedIn, waits for the HTTPS callback (up to 5 minutes),
/// then exchanges the code and stores the token securely.
///
/// Prerequisites:
///   - linkedin_client_id configured via save_linkedin_client_id
///   - linkedin_client_secret stored via save_linkedin_client_secret
///   - https://localhost:7892/callback registered in LinkedIn Developer Portal
///     → App → Auth → OAuth 2.0 settings → Authorized redirect URLs
///
/// Note: PKCE support must be enabled for your LinkedIn app in the Developer Portal.
#[tauri::command]
pub async fn start_linkedin_oauth_flow(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectedAccount, String> {
    use tauri_plugin_opener::OpenerExt;

    // 1. Load client credentials from secure storage
    let client_id = crate::db::settings_db::get(&state.db, "linkedin_client_id")
        .await
        .ok_or("LinkedIn Client ID not configured. Add it in Settings → Comptes.")?;

    let client_secret = crate::ai_keys::get_key("linkedin_client_secret").map_err(|_| {
        "LinkedIn client secret not configured. Add it in Settings → Comptes.".to_string()
    })?;

    // 2. PKCE
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let csrf = generate_csrf_state();

    // 3. Bind to port 7892 — separate from Instagram's 7891 to allow parallel auth flows
    const CALLBACK_PORT: u16 = 7892;
    let listener = TcpListener::bind(format!("127.0.0.1:{CALLBACK_PORT}"))
        .await
        .map_err(|e| {
            format!(
                "Failed to start callback server on port {CALLBACK_PORT}: {e}. \
                 Is another LinkedIn auth already in progress?"
            )
        })?;
    let redirect_uri = format!("https://localhost:{CALLBACK_PORT}/callback");

    // 4. TLS (self-signed cert, same as Instagram flow)
    let acceptor = build_tls_acceptor()?;

    // 5. Build LinkedIn authorization URL
    // Scopes: openid + profile (OIDC via "Sign In with LinkedIn using OpenID Connect" product)
    //         w_member_social ("Share on LinkedIn" product)
    // r_liteprofile intentionally excluded — legacy API v1 scope, conflicts with OIDC approach
    let scope = urlencoding::encode("openid profile w_member_social");
    let auth_url = format!(
        "https://www.linkedin.com/oauth/v2/authorization\
         ?response_type=code\
         &client_id={client_id}\
         &redirect_uri={encoded_redirect}\
         &scope={scope}\
         &state={csrf}\
         &code_challenge={code_challenge}\
         &code_challenge_method=S256",
        encoded_redirect = urlencoding::encode(&redirect_uri),
    );

    // 6. Open browser
    app.opener()
        .open_url(&auth_url, None::<&str>)
        .map_err(|e| format!("Failed to open browser for LinkedIn auth: {e}"))?;

    // 7. Wait for HTTPS callback (5 min timeout)
    let code = tokio::time::timeout(
        Duration::from_secs(300),
        accept_oauth_callback(listener, acceptor, &csrf),
    )
    .await
    .map_err(|_| "LinkedIn OAuth flow timed out — please try again")??;

    // 8. Exchange code → access token
    let access_token =
        crate::adapters::linkedin::exchange_code(&client_id, &client_secret, &code, &redirect_uri)
            .await?;

    // 9. Fetch profile (id + name) to build the author URN
    let user_info = crate::adapters::linkedin::get_user_info(&access_token).await?;
    let display_name = user_info.display_name();

    // 10. Store token (never passes to renderer)
    let token_key = format!("linkedin:{}", user_info.id());
    crate::token_store::save_token(&token_key, &access_token)?;

    // 11. Persist account metadata to SQLite
    // LinkedIn has no public "username" — use display_name as the label
    let account = crate::db::accounts::upsert_and_get(
        &state.db,
        "linkedin",
        user_info.id(),
        &display_name,
        Some(&display_name),
        &token_key,
    )
    .await?;

    Ok(account.into())
}

/// Save the LinkedIn App Client ID to settings.
#[tauri::command]
pub async fn save_linkedin_client_id(
    client_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    crate::db::settings_db::set(&state.db, "linkedin_client_id", &client_id).await
}

/// Get the LinkedIn App Client ID from settings (for display in UI only).
#[tauri::command]
pub async fn get_linkedin_client_id(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    Ok(crate::db::settings_db::get(&state.db, "linkedin_client_id").await)
}

/// Save the LinkedIn app client_secret.
/// SECURITY: stored in api_keys.json (user data dir), never crosses IPC back to renderer.
#[tauri::command]
pub fn save_linkedin_client_secret(secret: String) -> Result<(), String> {
    crate::ai_keys::save_key("linkedin_client_secret", &secret)
}

/// Check if the LinkedIn client_secret is configured.
#[tauri::command]
pub fn get_linkedin_client_secret_status() -> bool {
    crate::ai_keys::has_key("linkedin_client_secret")
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

    #[test]
    fn tls_acceptor_builds_successfully() {
        // rustls needs a process-level CryptoProvider — install ring if not already set
        let _ = rustls::crypto::ring::default_provider().install_default();
        let result = build_tls_acceptor();
        assert!(
            result.is_ok(),
            "TLS acceptor should build: {:?}",
            result.err()
        );
    }

    // ── CSRF / security ──────────────────────────────────────────────────

    #[test]
    fn csrf_state_is_unique_each_call() {
        let s1 = generate_csrf_state();
        let s2 = generate_csrf_state();
        assert_ne!(s1, s2, "CSRF states must be unique");
    }

    #[test]
    fn csrf_state_has_minimum_length() {
        // 16 random bytes → 22 base64url chars (no-pad)
        let state = generate_csrf_state();
        assert!(state.len() >= 20, "CSRF state too short: {}", state.len());
    }

    #[test]
    fn code_verifier_is_unique_each_call() {
        let v1 = generate_code_verifier();
        let v2 = generate_code_verifier();
        assert_ne!(v1, v2, "PKCE verifiers must be unique");
    }

    #[test]
    fn code_verifier_no_padding() {
        let verifier = generate_code_verifier();
        assert!(!verifier.contains('='), "verifier must be no-pad base64url");
        assert!(
            !verifier.contains('+'),
            "verifier must use URL-safe alphabet"
        );
        assert!(
            !verifier.contains('/'),
            "verifier must use URL-safe alphabet"
        );
    }

    #[test]
    fn pkce_challenge_differs_from_verifier() {
        let verifier = generate_code_verifier();
        let challenge = generate_code_challenge(&verifier);
        assert_ne!(verifier, challenge, "challenge must differ from verifier");
    }

    // ── parse_query_param — injection / edge cases ───────────────────────

    #[test]
    fn parse_query_param_rejects_missing_http_version() {
        // Malformed request line (no "HTTP/") must return None gracefully
        let line = "GET /callback?code=abc123&state=xyz";
        assert_eq!(parse_query_param(line, "code"), None);
    }

    #[test]
    fn parse_query_param_handles_empty_value() {
        let line = "GET /callback?code=&state=xyz HTTP/1.1";
        assert_eq!(parse_query_param(line, "code"), Some("".to_string()));
    }

    #[test]
    fn parse_query_param_handles_multiple_equals() {
        // Value contains '=' — must not split on it
        let line = "GET /callback?code=abc=def&state=xyz HTTP/1.1";
        assert_eq!(parse_query_param(line, "code"), Some("abc=def".to_string()));
    }

    #[test]
    fn parse_query_param_returns_none_for_absent_param() {
        let line = "GET /callback?state=xyz HTTP/1.1";
        assert_eq!(parse_query_param(line, "code"), None);
    }

    #[test]
    fn parse_query_param_does_not_confuse_partial_match() {
        // "code_extra" must not match query for "code"
        let line = "GET /callback?code_extra=abc&state=xyz HTTP/1.1";
        assert_eq!(parse_query_param(line, "code"), None);
    }

    // ── parse_hex_color ─────────────────────────────────────────────────

    #[test]
    fn parse_hex_color_accepts_valid_three_and_six_digit_hex() {
        assert_eq!(parse_hex_color("#abc").unwrap(), Some("#abc"));
        assert_eq!(parse_hex_color("#ABC").unwrap(), Some("#ABC"));
        assert_eq!(parse_hex_color("#3ddc84").unwrap(), Some("#3ddc84"));
        assert_eq!(parse_hex_color("#0D9488").unwrap(), Some("#0D9488"));
    }

    #[test]
    fn parse_hex_color_treats_empty_and_whitespace_as_clear() {
        assert_eq!(parse_hex_color("").unwrap(), None);
        assert_eq!(parse_hex_color("   ").unwrap(), None);
    }

    #[test]
    fn parse_hex_color_trims_surrounding_whitespace() {
        assert_eq!(parse_hex_color("  #abc  ").unwrap(), Some("#abc"));
    }

    #[test]
    fn parse_hex_color_rejects_non_hex_characters_inside_the_string() {
        // Critical: these would otherwise be interpolated into <style> blocks
        // and could break out of the CSS context.
        for malformed in [
            "#g00",    // invalid hex digit
            "#zzzzzz", // all invalid
            "#ab\"}>", // 6 chars but quote/brace/gt
            "#a;b}c",  // semicolon — would terminate CSS rule
            "#'><svg", // angle brackets and quotes
            "#abc; }", // 7 chars but with space/semi
        ] {
            assert!(
                parse_hex_color(malformed).is_err(),
                "must reject malformed input {malformed:?}"
            );
        }
    }

    #[test]
    fn parse_hex_color_rejects_wrong_length() {
        for len_invalid in ["#a", "#ab", "#abcd", "#abcde", "#abcdefg"] {
            assert!(
                parse_hex_color(len_invalid).is_err(),
                "must reject bad length {len_invalid:?}"
            );
        }
    }

    #[test]
    fn parse_hex_color_rejects_missing_hash_prefix() {
        assert!(parse_hex_color("3ddc84").is_err());
        assert!(parse_hex_color("abc").is_err());
    }
}
