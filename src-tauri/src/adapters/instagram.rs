use serde::Deserialize;

const TOKEN_URL: &str = "https://api.instagram.com/oauth/access_token";
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
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            return Err(err.error_message.or(err.error_description).unwrap_or(body));
        }
        return Err(format!("Token exchange failed: {body}"));
    }

    let token: TokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {e}"))?;

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
