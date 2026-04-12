use serde::{Deserialize, Serialize};

const TOKEN_URL: &str = "https://www.linkedin.com/oauth/v2/accessToken";
const API_BASE: &str = "https://api.linkedin.com/v2";

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LinkedInUser {
    pub id: String,
    #[serde(rename = "localizedFirstName")]
    pub first_name: Option<String>,
    #[serde(rename = "localizedLastName")]
    pub last_name: Option<String>,
}

impl LinkedInUser {
    /// Build a display name from first + last names, falling back to the profile ID.
    pub fn display_name(&self) -> String {
        match (&self.first_name, &self.last_name) {
            (Some(f), Some(l)) => format!("{f} {l}"),
            (Some(f), None) => f.clone(),
            (None, Some(l)) => l.clone(),
            (None, None) => self.id.clone(),
        }
    }
}

// ── Token exchange ─────────────────────────────────────────────────────────

/// Exchange an authorization code for a LinkedIn access token (PKCE + client_secret).
/// LinkedIn requires client_secret even when PKCE is used.
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
        error_description: Option<String>,
        error: Option<String>,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| format!("Network error during LinkedIn token exchange: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            return Err(err.error_description.or(err.error).unwrap_or(body));
        }
        return Err(format!("LinkedIn token exchange failed: {body}"));
    }

    let token: TokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse LinkedIn token response: {e}"))?;

    Ok(token.access_token)
}

// ── User info ──────────────────────────────────────────────────────────────

/// Fetch basic profile info (id, first/last name) to build the author URN.
/// Requires `openid profile r_liteprofile` scopes.
pub async fn get_user_info(access_token: &str) -> Result<LinkedInUser, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{API_BASE}/me"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Network error fetching LinkedIn user info: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Failed to get LinkedIn user info: {body}"));
    }

    resp.json()
        .await
        .map_err(|e| format!("Failed to parse LinkedIn user info: {e}"))
}

// ── Image upload ───────────────────────────────────────────────────────────

/// Step 1 — Register an image upload with the LinkedIn Assets API.
/// Returns `(upload_url, asset_urn)`.
///
/// LinkedIn uploads require two steps:
///   1. Register → obtain an upload URL + asset URN
///   2. PUT raw bytes to the upload URL
///   3. Reference the asset URN in the ugcPost body
pub async fn register_image_upload(
    profile_id: &str,
    access_token: &str,
) -> Result<(String, String), String> {
    // ── Request body ──────────────────────────────────────────────────────
    #[derive(Serialize)]
    struct RegisterRequest {
        #[serde(rename = "registerUploadRequest")]
        register_upload_request: RegisterUploadRequest,
    }

    #[derive(Serialize)]
    struct RegisterUploadRequest {
        recipes: Vec<String>,
        owner: String,
        #[serde(rename = "serviceRelationships")]
        service_relationships: Vec<ServiceRelationship>,
    }

    #[derive(Serialize)]
    struct ServiceRelationship {
        #[serde(rename = "relationshipType")]
        relationship_type: String,
        identifier: String,
    }

    // ── Response body ─────────────────────────────────────────────────────
    #[derive(Deserialize)]
    struct RegisterResponse {
        value: RegisterValue,
    }

    #[derive(Deserialize)]
    struct RegisterValue {
        #[serde(rename = "uploadMechanism")]
        upload_mechanism: UploadMechanism,
        asset: String,
    }

    #[derive(Deserialize)]
    struct UploadMechanism {
        #[serde(rename = "com.linkedin.digitalmedia.uploading.MediaUploadHttpRequest")]
        http_request: UploadHttpRequest,
    }

    #[derive(Deserialize)]
    struct UploadHttpRequest {
        #[serde(rename = "uploadUrl")]
        upload_url: String,
    }

    let body = RegisterRequest {
        register_upload_request: RegisterUploadRequest {
            recipes: vec!["urn:li:digitalmediaRecipe:feedshare-image".to_string()],
            owner: format!("urn:li:person:{profile_id}"),
            service_relationships: vec![ServiceRelationship {
                relationship_type: "OWNER".to_string(),
                identifier: "urn:li:userGeneratedContent".to_string(),
            }],
        },
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{API_BASE}/assets?action=registerUpload"))
        .bearer_auth(access_token)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("LinkedIn registerUpload network error: {e}"))?;

    if !resp.status().is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("LinkedIn registerUpload failed: {body_text}"));
    }

    let reg: RegisterResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse LinkedIn registerUpload response: {e}"))?;

    Ok((
        reg.value.upload_mechanism.http_request.upload_url,
        reg.value.asset,
    ))
}

/// Step 2 — PUT raw image bytes to the LinkedIn upload URL.
/// LinkedIn accepts JPEG or PNG (max 10 MB).
pub async fn upload_image_binary(
    upload_url: &str,
    image_bytes: &[u8],
    access_token: &str,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .put(upload_url)
        .bearer_auth(access_token)
        .header("Content-Type", "application/octet-stream")
        .body(image_bytes.to_vec())
        .send()
        .await
        .map_err(|e| format!("LinkedIn image upload network error: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("LinkedIn image upload failed ({status}): {body}"));
    }

    Ok(())
}

// ── Publishing ─────────────────────────────────────────────────────────────

/// Publish a text-only ugcPost.
/// Hashtags must be embedded in `text` (LinkedIn API has no separate hashtag field).
/// Returns the created ugcPost ID/URN.
pub async fn publish_text(
    profile_id: &str,
    text: &str,
    access_token: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
        "author": format!("urn:li:person:{profile_id}"),
        "lifecycleState": "PUBLISHED",
        "specificContent": {
            "com.linkedin.ugc.ShareContent": {
                "shareCommentary": { "text": text },
                "shareMediaCategory": "NONE"
            }
        },
        "visibility": {
            "com.linkedin.ugc.MemberNetworkVisibility": "PUBLIC"
        }
    });

    post_ugc(body, access_token).await
}

/// Publish an image ugcPost.
/// `asset_urn` must come from `register_image_upload` after the binary upload completes.
/// Hashtags must be embedded in `text` (LinkedIn has no separate hashtag field).
/// Returns the created ugcPost ID/URN.
pub async fn publish_image(
    profile_id: &str,
    text: &str,
    asset_urn: &str,
    access_token: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
        "author": format!("urn:li:person:{profile_id}"),
        "lifecycleState": "PUBLISHED",
        "specificContent": {
            "com.linkedin.ugc.ShareContent": {
                "shareCommentary": { "text": text },
                "shareMediaCategory": "IMAGE",
                "media": [{
                    "status": "READY",
                    "media": asset_urn
                }]
            }
        },
        "visibility": {
            "com.linkedin.ugc.MemberNetworkVisibility": "PUBLIC"
        }
    });

    post_ugc(body, access_token).await
}

/// Internal helper — POST a ugcPost body and return the created post URN.
/// LinkedIn returns the post URN in the `x-restli-id` response header
/// when using REST-LI protocol version 2.0.0.
async fn post_ugc(body: serde_json::Value, access_token: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{API_BASE}/ugcPosts"))
        .bearer_auth(access_token)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("LinkedIn ugcPost network error: {e}"))?;

    // Capture header before consuming the response body
    let status = resp.status();
    let post_urn = resp
        .headers()
        .get("x-restli-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("LinkedIn publish failed ({status}): {body_text}"));
    }

    Ok(post_urn)
}
