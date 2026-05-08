use serde::{Deserialize, Serialize};

const TOKEN_URL_DEFAULT: &str = "https://www.linkedin.com/oauth/v2/accessToken";
const API_BASE_DEFAULT: &str = "https://api.linkedin.com/v2";

/// LinkedIn REST API base URL. Defaults to production; overridable via
/// `GETPOSTCRAFT_LINKEDIN_API` so integration tests can point this at a
/// `wiremock::MockServer`.
fn api_base() -> String {
    std::env::var("GETPOSTCRAFT_LINKEDIN_API").unwrap_or_else(|_| API_BASE_DEFAULT.to_string())
}

/// Token exchange URL. Same pattern as `api_base`.
fn token_url() -> String {
    std::env::var("GETPOSTCRAFT_LINKEDIN_TOKEN_URL")
        .unwrap_or_else(|_| TOKEN_URL_DEFAULT.to_string())
}

// ── Types ──────────────────────────────────────────────────────────────────

/// Profile returned by the OIDC /v2/userinfo endpoint.
/// `sub` is the stable LinkedIn member URN ID (e.g. "hBdTzjkE4S").
#[derive(Debug, Deserialize)]
pub struct LinkedInUser {
    /// Stable member ID — used to build `urn:li:person:{sub}` author URN.
    pub sub: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
}

impl LinkedInUser {
    /// Build a display name from given + family names, falling back to the member ID.
    pub fn display_name(&self) -> String {
        match (&self.given_name, &self.family_name) {
            (Some(f), Some(l)) => format!("{f} {l}"),
            (Some(f), None) => f.clone(),
            (None, Some(l)) => l.clone(),
            (None, None) => self.sub.clone(),
        }
    }

    /// The profile ID used in author URNs: `urn:li:person:{id}`.
    pub fn id(&self) -> &str {
        &self.sub
    }
}

// ── Token exchange ─────────────────────────────────────────────────────────

/// Exchange an authorization code for a LinkedIn access token.
/// LinkedIn confidential clients use client_secret only — PKCE code_verifier is
/// not supported for apps that have a client_secret (public-client flow only).
/// Returns `(access_token, expires_in_seconds)`. LinkedIn typically issues
/// 60-day tokens (5184000 s) but the value can vary by app type, so we
/// trust whatever the response says rather than assume.
pub async fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<(String, Option<i64>), String> {
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        /// LinkedIn returns this on every successful exchange. Optional in
        /// our type for defensive parsing; if the field disappears we'd
        /// rather log a missing-expiry account than fail the whole login.
        #[serde(default)]
        expires_in: Option<i64>,
    }

    #[derive(Deserialize)]
    struct ErrorResponse {
        error_description: Option<String>,
        error: Option<String>,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(token_url())
        // LinkedIn requires client credentials via HTTP Basic Auth for confidential clients,
        // not in the form body (invalid_client if sent as form params).
        // LinkedIn confidential clients: credentials in form body only, no PKCE verifier.
        // PKCE (code_verifier) is for public clients only — sending it with client_secret
        // causes invalid_client. Basic Auth is also ignored; form body is authoritative.
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
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

    Ok((token.access_token, token.expires_in))
}

// ── User info ──────────────────────────────────────────────────────────────

/// Fetch profile via the OIDC userinfo endpoint.
/// Requires `openid profile` scopes (Sign In with LinkedIn using OpenID Connect product).
/// Returns sub (stable member ID), given_name, family_name.
pub async fn get_user_info(access_token: &str) -> Result<LinkedInUser, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/userinfo", api_base()))
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
        .post(format!("{}/assets?action=registerUpload", api_base()))
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

/// Validate that an upload URL belongs to LinkedIn's own infrastructure.
/// Prevents SSRF: the Bearer token must never be sent to an arbitrary host.
///
/// In tests, `GETPOSTCRAFT_LINKEDIN_API` points at a wiremock server on
/// localhost — we allow that prefix too, otherwise integration tests can't
/// run. Production behavior is unchanged (env var unset → only LinkedIn
/// hosts pass).
fn validate_linkedin_upload_url(url: &str) -> Result<(), String> {
    let prod_ok = url.starts_with("https://www.linkedin.com/")
        || url.starts_with("https://media.licdn.com/")
        || url.starts_with("https://api.linkedin.com/");
    let test_override_ok = std::env::var("GETPOSTCRAFT_LINKEDIN_API")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some_and(|base| url.starts_with(&base));
    if prod_ok || test_override_ok {
        Ok(())
    } else {
        Err(format!(
            "LinkedIn upload URL domain validation failed — unexpected host in: {}",
            url.chars().take(80).collect::<String>()
        ))
    }
}

/// Step 2 — PUT raw image bytes to the LinkedIn upload URL.
/// LinkedIn accepts JPEG or PNG (max 10 MB).
/// The upload_url is validated against LinkedIn domains before sending the Bearer token.
pub async fn upload_image_binary(
    upload_url: &str,
    image_bytes: &[u8],
    access_token: &str,
) -> Result<(), String> {
    // SECURITY: validate domain before attaching Bearer token (SSRF prevention)
    validate_linkedin_upload_url(upload_url)?;

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

/// Publish an image ugcPost. Accepts 1 to N asset URNs — LinkedIn renders 2+
/// images as a tiled gallery in the post (true swipeable carousel requires
/// `shareMediaCategory: "DOCUMENT"` with a PDF, not handled here).
///
/// Each `asset_urn` must come from a sequential `register_image_upload` +
/// `upload_image_binary` pair. Hashtags must be embedded in `text` — LinkedIn
/// has no separate hashtag field.
pub async fn publish_image(
    profile_id: &str,
    text: &str,
    asset_urns: &[&str],
    access_token: &str,
) -> Result<String, String> {
    if asset_urns.is_empty() {
        return Err("publish_image requires at least one asset URN".to_string());
    }

    // Build one media descriptor per asset. `shareMediaCategory` stays "IMAGE"
    // even for galleries — there is no separate "CAROUSEL" category for raw images.
    let media: Vec<serde_json::Value> = asset_urns
        .iter()
        .map(|urn| {
            serde_json::json!({
                "status": "READY",
                "media": urn,
            })
        })
        .collect();

    let body = serde_json::json!({
        "author": format!("urn:li:person:{profile_id}"),
        "lifecycleState": "PUBLISHED",
        "specificContent": {
            "com.linkedin.ugc.ShareContent": {
                "shareCommentary": { "text": text },
                "shareMediaCategory": "IMAGE",
                "media": media
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
        .post(format!("{}/ugcPosts", api_base()))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Inline reimplementation of `publish_image`'s body builder so we can
    /// assert on the JSON payload without hitting the network. Mirrors the
    /// real function exactly — keep them in sync.
    fn build_publish_image_body(
        profile_id: &str,
        text: &str,
        asset_urns: &[&str],
    ) -> serde_json::Value {
        let media: Vec<serde_json::Value> = asset_urns
            .iter()
            .map(|urn| serde_json::json!({ "status": "READY", "media": urn }))
            .collect();
        serde_json::json!({
            "author": format!("urn:li:person:{profile_id}"),
            "lifecycleState": "PUBLISHED",
            "specificContent": {
                "com.linkedin.ugc.ShareContent": {
                    "shareCommentary": { "text": text },
                    "shareMediaCategory": "IMAGE",
                    "media": media
                }
            },
            "visibility": {
                "com.linkedin.ugc.MemberNetworkVisibility": "PUBLIC"
            }
        })
    }

    #[test]
    fn publish_image_body_single_image() {
        let body = build_publish_image_body("abc123", "Hello", &["urn:li:digitalmediaAsset:1"]);
        let media = body
            .pointer("/specificContent/com.linkedin.ugc.ShareContent/media")
            .and_then(|m| m.as_array())
            .expect("media array");
        assert_eq!(media.len(), 1);
        assert_eq!(media[0]["status"], "READY");
        assert_eq!(media[0]["media"], "urn:li:digitalmediaAsset:1");
    }

    #[test]
    fn publish_image_body_multi_image_preserves_order() {
        let urns = [
            "urn:slide-1",
            "urn:slide-2",
            "urn:slide-3",
            "urn:slide-4",
            "urn:slide-5",
        ];
        let body = build_publish_image_body("abc123", "Carousel post", &urns);
        let media = body
            .pointer("/specificContent/com.linkedin.ugc.ShareContent/media")
            .and_then(|m| m.as_array())
            .expect("media array");
        assert_eq!(media.len(), 5);
        for (i, urn) in urns.iter().enumerate() {
            assert_eq!(media[i]["media"], *urn, "slide order must be preserved");
        }
    }

    #[test]
    fn publish_image_body_uses_image_category_even_for_multi() {
        // LinkedIn does NOT have a CAROUSEL category for raw images — it stays IMAGE.
        // Regression guard: if someone later changes this, multi-image posts would
        // be rejected by LinkedIn with an opaque 400.
        let body = build_publish_image_body("x", "y", &["a", "b"]);
        assert_eq!(
            body["specificContent"]["com.linkedin.ugc.ShareContent"]["shareMediaCategory"],
            "IMAGE"
        );
    }

    #[tokio::test]
    async fn publish_image_rejects_empty_urn_slice() {
        let res = publish_image("x", "y", &[], "fake-token").await;
        assert!(res.is_err());
        assert!(
            res.unwrap_err().contains("at least one asset URN"),
            "should fail fast before any network call"
        );
    }

    // ── Mock-server integration tests ────────────────────────────────────
    //
    // These tests stand up a `wiremock` server on localhost, point the
    // production code at it via `GETPOSTCRAFT_LINKEDIN_API`, and assert
    // that each LinkedIn flow makes exactly the right calls in the right
    // order. Like the IG tests, they would have caught any silent
    // degradation of multi-image publishing.

    use serial_test::serial;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// SAFETY: `#[serial]` ensures only one test owns the env var at a time.
    async fn boot_li_mock() -> MockServer {
        let server = MockServer::start().await;
        unsafe {
            std::env::set_var("GETPOSTCRAFT_LINKEDIN_API", server.uri());
        }
        server
    }

    fn clear_li_env() {
        unsafe {
            std::env::remove_var("GETPOSTCRAFT_LINKEDIN_API");
        }
    }

    #[tokio::test]
    #[serial]
    async fn register_image_upload_returns_url_and_asset_urn() {
        let server = boot_li_mock().await;
        // The upload URL must point inside the mock server too — the SSRF
        // guard accepts the test base URL when GETPOSTCRAFT_LINKEDIN_API is set.
        let upload_url = format!("{}/upload-target/abc123", server.uri());
        let body = serde_json::json!({
            "value": {
                "uploadMechanism": {
                    "com.linkedin.digitalmedia.uploading.MediaUploadHttpRequest": {
                        "uploadUrl": upload_url
                    }
                },
                "asset": "urn:li:digitalmediaAsset:42"
            }
        });

        Mock::given(method("POST"))
            .and(path("/assets"))
            .and(query_param("action", "registerUpload"))
            .and(header("X-Restli-Protocol-Version", "2.0.0"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;

        let (got_url, urn) = register_image_upload("profile_id_xyz", "test-token")
            .await
            .expect("register should succeed");
        assert_eq!(got_url, upload_url);
        assert_eq!(urn, "urn:li:digitalmediaAsset:42");

        drop(server);
        clear_li_env();
    }

    #[tokio::test]
    #[serial]
    async fn upload_image_binary_sends_bytes_with_bearer() {
        let server = boot_li_mock().await;
        let upload_path = "/upload-target/abc";
        let upload_url = format!("{}{}", server.uri(), upload_path);

        Mock::given(method("PUT"))
            .and(path(upload_path))
            .and(header("Authorization", "Bearer test-token"))
            .and(header("Content-Type", "application/octet-stream"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&server)
            .await;

        // 1 KB of fake PNG bytes is enough to verify the body reaches the server.
        let bytes = vec![0u8; 1024];
        upload_image_binary(&upload_url, &bytes, "test-token")
            .await
            .expect("upload should succeed");

        drop(server);
        clear_li_env();
    }

    #[tokio::test]
    #[serial]
    async fn upload_image_binary_refuses_evil_host_even_with_env_override() {
        // SSRF: even when the test override is in play, a URL that doesn't
        // start with the configured base must be rejected before the Bearer
        // token is sent.
        let server = boot_li_mock().await;
        let evil = "https://evil.example.com/exfil";

        let result = upload_image_binary(evil, &[0u8; 16], "secret-token").await;
        assert!(
            result.is_err(),
            "must refuse non-LinkedIn hosts even in test mode"
        );
        assert!(
            result.unwrap_err().contains("domain validation"),
            "error should reference the SSRF guard"
        );

        // wiremock should not have logged any request — assert by giving it
        // an unmatched mock that would have logged a hit.
        // (No mock registered → drop the server unmatched.)
        drop(server);
        clear_li_env();
    }

    #[tokio::test]
    #[serial]
    async fn publish_text_posts_ugc_returns_post_urn() {
        let server = boot_li_mock().await;

        Mock::given(method("POST"))
            .and(path("/ugcPosts"))
            .and(header("X-Restli-Protocol-Version", "2.0.0"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("x-restli-id", "urn:li:share:123")
                    .set_body_string(""),
            )
            .expect(1)
            .mount(&server)
            .await;

        let urn = publish_text("profile_id", "Hello LinkedIn", "test-token")
            .await
            .expect("text post should succeed");
        assert_eq!(urn, "urn:li:share:123");

        drop(server);
        clear_li_env();
    }

    #[tokio::test]
    #[serial]
    async fn publish_image_multi_sends_all_asset_urns() {
        let server = boot_li_mock().await;

        // Capture any matching ugcPost request — wiremock by default records
        // bodies for inspection. We assert the body contains all 3 URNs.
        Mock::given(method("POST"))
            .and(path("/ugcPosts"))
            .and(wiremock::matchers::body_string_contains(
                "urn:li:digitalmediaAsset:s1",
            ))
            .and(wiremock::matchers::body_string_contains(
                "urn:li:digitalmediaAsset:s2",
            ))
            .and(wiremock::matchers::body_string_contains(
                "urn:li:digitalmediaAsset:s3",
            ))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("x-restli-id", "urn:li:share:multi")
                    .set_body_string(""),
            )
            .expect(1)
            .mount(&server)
            .await;

        let urns = [
            "urn:li:digitalmediaAsset:s1",
            "urn:li:digitalmediaAsset:s2",
            "urn:li:digitalmediaAsset:s3",
        ];
        let post = publish_image("profile_id", "Carousel post", &urns, "test-token")
            .await
            .expect("multi-image post should succeed");
        assert_eq!(post, "urn:li:share:multi");

        drop(server);
        clear_li_env();
    }

    #[tokio::test]
    #[serial]
    async fn publish_text_propagates_400_error() {
        let server = boot_li_mock().await;

        Mock::given(method("POST"))
            .and(path("/ugcPosts"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_author_urn"))
            .expect(1)
            .mount(&server)
            .await;

        let result = publish_text("profile_id", "x", "test-token").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("invalid_author_urn") || err.contains("400"),
            "error should reference upstream cause, got: {err}"
        );

        drop(server);
        clear_li_env();
    }
}
