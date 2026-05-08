use serde::Deserialize;

const TOKEN_URL: &str = "https://api.instagram.com/oauth/access_token";
const LONG_LIVED_URL: &str = "https://graph.instagram.com/access_token";
const USER_INFO_URL: &str = "https://graph.instagram.com/me";

#[derive(Debug, Deserialize)]
pub struct InstagramUser {
    pub id: String,
    pub username: String,
    pub name: Option<String>,
}

/// Exchange an authorization code for an access token using PKCE.
/// Meta requires client_secret even when using PKCE (non-standard but enforced).
pub async fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    #[derive(Deserialize)]
    struct ErrorResponse {
        error_message: Option<String>,
        error_description: Option<String>,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("Network error during token exchange: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        // Try to surface the structured error message first; if Meta echoes
        // anything secret-looking in the fallback raw body, scrub it before
        // bubbling the message up (it can land in user-visible error toasts
        // and crash reports).
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            return Err(err
                .error_message
                .or(err.error_description)
                .unwrap_or_else(|| crate::log_redact::redact_secrets(&body)));
        }
        return Err(format!(
            "Token exchange failed: {}",
            crate::log_redact::redact_secrets(&body)
        ));
    }

    let token: TokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {e}"))?;

    Ok(token.access_token)
}

/// Exchange a short-lived token for a long-lived token (valid ~60 days).
/// Must be called right after exchange_code — short-lived tokens expire in 1-2 hours.
///
/// Endpoint: GET https://graph.instagram.com/access_token
///   ?grant_type=ig_exchange_token
///   &client_secret={secret}
///   &access_token={short_lived}
pub async fn exchange_for_long_lived_token(
    short_lived_token: &str,
    client_secret: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct LongLivedResponse {
        access_token: String,
    }

    log::info!("Instagram: requesting long-lived token from {LONG_LIVED_URL}");

    let client = reqwest::Client::new();
    let resp = client
        .get(LONG_LIVED_URL)
        .query(&[
            ("grant_type", "ig_exchange_token"),
            ("client_secret", client_secret),
            ("access_token", short_lived_token),
        ])
        .send()
        .await
        .map_err(|e| format!("Network error during long-lived token exchange: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let safe_body = crate::log_redact::redact_secrets(&body);
        log::warn!("Instagram: long-lived token exchange returned HTTP {status}: {safe_body}");
        return Err(format!("Long-lived token exchange failed: {safe_body}"));
    }

    let token: LongLivedResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse long-lived token response: {e}"))?;

    Ok(token.access_token)
}

/// Fetch basic profile info for the authenticated user.
pub async fn get_user_info(access_token: &str) -> Result<InstagramUser, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(USER_INFO_URL)
        .query(&[
            ("fields", "id,username,name"),
            ("access_token", access_token),
        ])
        .send()
        .await
        .map_err(|e| format!("Network error fetching user info: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Failed to get Instagram user info: {body}"));
    }

    resp.json()
        .await
        .map_err(|e| format!("Failed to parse user info: {e}"))
}

/// Verify a stored token is still valid by calling the user info endpoint.
/// Used by the publisher — kept for V1 publishing command.
#[allow(dead_code)]
pub async fn verify_token(access_token: &str) -> Result<(), String> {
    get_user_info(access_token).await.map(|_| ())
}
