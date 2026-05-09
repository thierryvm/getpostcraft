use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

const IG_API_DEFAULT: &str = "https://graph.instagram.com/v21.0";
const IMGBB_API: &str = "https://api.imgbb.com/1/upload";
const CATBOX_API: &str = "https://catbox.moe/user/api.php";
const TMPFILES_API: &str = "https://tmpfiles.org/api/v1/upload";
const NULLPTRME_API: &str = "https://0x0.st";

/// Returns the Instagram Graph API base URL. Defaults to the production
/// endpoint; overridable via `GETPOSTCRAFT_IG_API` so integration tests can
/// point this at a `wiremock::MockServer` without recompiling.
fn ig_api_base() -> String {
    std::env::var("GETPOSTCRAFT_IG_API").unwrap_or_else(|_| IG_API_DEFAULT.to_string())
}

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishResult {
    pub post_id: i64,
    /// Platform-specific media ID or post URN (Instagram media ID, LinkedIn ugcPost URN, etc.)
    pub media_id: String,
    pub published_at: String,
}

// ── Image upload helpers ───────────────────────────────────────────────────

/// Decode an image source (base64 data URL or file path) to raw bytes.
fn decode_image_bytes(image_source: &str) -> Result<Vec<u8>, String> {
    if let Some(b64) = image_source.strip_prefix("data:image/png;base64,") {
        STANDARD
            .decode(b64)
            .map_err(|e| format!("Base64 decode error: {e}"))
    } else {
        std::fs::read(image_source)
            .map_err(|e| format!("Cannot read image file '{image_source}': {e}"))
    }
}

/// Upload an image to catbox.moe (no API key required).
/// Returns the public URL, e.g. `https://files.catbox.moe/xxxxxx.png`.
async fn upload_image_to_catbox(
    client: &reqwest::Client,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("image.png")
        .mime_str("image/png")
        .map_err(|e| format!("Catbox mime error: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .text("reqtype", "fileupload")
        .part("fileToUpload", part);

    let resp = client
        .post(CATBOX_API)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("Catbox network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Catbox upload failed: HTTP {}", resp.status()));
    }

    let url = resp
        .text()
        .await
        .map_err(|e| format!("Catbox response error: {e}"))?;
    let url = url.trim().to_string();

    if url.starts_with("https://") {
        Ok(url)
    } else {
        Err(format!("Catbox returned unexpected response: {url}"))
    }
}

/// Upload an image to 0x0.st (no API key required, reliable fallback).
/// Returns the public URL.
async fn upload_image_to_nullptrme(
    client: &reqwest::Client,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("image.png")
        .mime_str("image/png")
        .map_err(|e| format!("0x0.st mime error: {e}"))?;

    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(NULLPTRME_API)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("0x0.st network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("0x0.st upload failed: HTTP {}", resp.status()));
    }

    let url = resp
        .text()
        .await
        .map_err(|e| format!("0x0.st response error: {e}"))?;
    let url = url.trim().to_string();

    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(url)
    } else {
        Err(format!("0x0.st returned unexpected response: {url}"))
    }
}

/// Upload an image to tmpfiles.org (no API key, files expire after 1 hour).
async fn upload_image_to_tmpfiles(
    client: &reqwest::Client,
    bytes: Vec<u8>,
) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct TmpFilesData {
        url: String,
    }
    #[derive(serde::Deserialize)]
    struct TmpFilesResponse {
        status: String,
        data: Option<TmpFilesData>,
    }

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("image.png")
        .mime_str("image/png")
        .map_err(|e| format!("tmpfiles mime error: {e}"))?;

    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(TMPFILES_API)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("tmpfiles.org network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "tmpfiles.org upload failed: HTTP {}",
            resp.status()
        ));
    }

    let body: TmpFilesResponse = resp
        .json()
        .await
        .map_err(|e| format!("tmpfiles.org response parse error: {e}"))?;

    if body.status != "success" {
        return Err(format!("tmpfiles.org returned status: {}", body.status));
    }

    // tmpfiles returns https://tmpfiles.org/XXXXX/file.png
    // Instagram needs a direct download link → rewrite to /dl/ path
    let url = body
        .data
        .map(|d| d.url.replacen("tmpfiles.org/", "tmpfiles.org/dl/", 1))
        .ok_or_else(|| "tmpfiles.org returned no URL".to_string())?;

    Ok(url)
}

/// Upload image with automatic fallback chain:
///   Catbox → 0x0.st → tmpfiles.org
///
/// Litterbox (`litter.catbox.moe`) is intentionally excluded: Meta's CDN
/// rejects fetches from that host with error code 9004 / subcode 2207052
/// ("Only photo or video can be accepted as media type"), making it unusable
/// for Instagram even though it works for storage. Last-resort uploads should
/// rely on imgbb (configured by the user) rather than a host Meta is known
/// to refuse.
///
/// If all hosts fail, returns a consolidated error with hints to configure imgbb.
async fn upload_image_free(image_source: &str) -> Result<String, String> {
    let bytes = decode_image_bytes(image_source)?;
    let client = reqwest::Client::new();

    let mut errors: Vec<String> = Vec::new();

    macro_rules! try_host {
        ($name:expr, $fut:expr) => {
            match $fut.await {
                Ok(url) => return Ok(url),
                Err(e) => errors.push(format!("  {}: {}", $name, e)),
            }
        };
    }

    try_host!("Catbox", upload_image_to_catbox(&client, bytes.clone()));
    try_host!("0x0.st", upload_image_to_nullptrme(&client, bytes.clone()));
    try_host!("tmpfiles.org", upload_image_to_tmpfiles(&client, bytes));

    Err(format!(
        "Tous les hébergeurs gratuits sont indisponibles :\n{}\n\n\
         Solution : configure une clé imgbb dans Paramètres → Publication \
         (imgbb.com → API, compte gratuit).",
        errors.join("\n")
    ))
}

/// Upload an image to imgbb and return the public URL.
/// `image_source` can be either:
///   - a local file path (absolute)
///   - a base64 data URL (`data:image/png;base64,...`) — as returned by the render pipeline
async fn upload_image_to_imgbb(image_source: &str, api_key: &str) -> Result<String, String> {
    let bytes = decode_image_bytes(image_source)?;
    let b64 = STANDARD.encode(&bytes);

    #[derive(Deserialize)]
    struct ImgbbData {
        url: String,
    }
    #[derive(Deserialize)]
    struct ImgbbResponse {
        success: bool,
        data: Option<ImgbbData>,
        error: Option<serde_json::Value>,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(IMGBB_API)
        .query(&[("key", api_key)])
        .form(&[("image", &b64)])
        .send()
        .await
        .map_err(|e| format!("imgbb network error: {e}"))?;

    let body: ImgbbResponse = resp
        .json()
        .await
        .map_err(|e| format!("imgbb response parse error: {e}"))?;

    if !body.success {
        return Err(format!(
            "imgbb upload failed: {:?}",
            body.error.unwrap_or_default()
        ));
    }

    body.data
        .map(|d| d.url)
        .ok_or_else(|| "imgbb returned no URL".to_string())
}

// ── Instagram Graph API ────────────────────────────────────────────────────

/// Single-image container — used for normal posts AND as the leaf step of the
/// carousel flow (one container per slide before assembling the parent).
async fn create_ig_container(
    ig_user_id: &str,
    image_url: &str,
    caption: &str,
    access_token: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct ContainerResponse {
        id: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/{ig_user_id}/media", ig_api_base()))
        .form(&[
            ("image_url", image_url),
            ("caption", caption),
            ("access_token", access_token),
        ])
        .send()
        .await
        .map_err(|e| format!("Instagram container creation network error: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Instagram container creation failed: {body}"));
    }

    let r: ContainerResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse container response: {e}"))?;
    Ok(r.id)
}

/// Carousel item container — one per slide. Differs from the single-image
/// container by `is_carousel_item=true` and the absence of a caption (the
/// caption lives only on the parent CAROUSEL container).
async fn create_ig_carousel_item(
    ig_user_id: &str,
    image_url: &str,
    access_token: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct ContainerResponse {
        id: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/{ig_user_id}/media", ig_api_base()))
        .form(&[
            ("image_url", image_url),
            ("is_carousel_item", "true"),
            ("access_token", access_token),
        ])
        .send()
        .await
        .map_err(|e| format!("Instagram carousel item network error: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Instagram carousel item creation failed: {body}"));
    }

    let r: ContainerResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse carousel item response: {e}"))?;
    Ok(r.id)
}

/// Carousel parent container — references the children created by
/// `create_ig_carousel_item` and carries the post caption.
async fn create_ig_carousel_parent(
    ig_user_id: &str,
    children_ids: &[String],
    caption: &str,
    access_token: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct ContainerResponse {
        id: String,
    }

    let children = children_ids.join(",");
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/{ig_user_id}/media", ig_api_base()))
        .form(&[
            ("media_type", "CAROUSEL"),
            ("children", &children),
            ("caption", caption),
            ("access_token", access_token),
        ])
        .send()
        .await
        .map_err(|e| format!("Instagram carousel parent network error: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Instagram carousel parent creation failed: {body}"));
    }

    let r: ContainerResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse carousel parent response: {e}"))?;
    Ok(r.id)
}

/// Fetch the public permalink (`https://www.instagram.com/p/{shortcode}/`) of
/// a freshly-published Instagram media. The publish response only returns the
/// numeric media id — the shortcode that builds the public URL lives on a
/// follow-up `?fields=permalink` call.
///
/// Best-effort: a failure here returns Err and the caller is expected to
/// log + continue without the URL (the post is still successfully published,
/// the user just won't get a deep link in the UI). Times out at 10 s so a
/// transient Graph API hiccup doesn't stall the publish flow.
async fn fetch_ig_permalink(media_id: &str, access_token: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct PermalinkResponse {
        permalink: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/{media_id}", ig_api_base()))
        .query(&[("fields", "permalink"), ("access_token", access_token)])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("permalink fetch network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("permalink fetch HTTP {}", resp.status()));
    }

    let body: PermalinkResponse = resp
        .json()
        .await
        .map_err(|e| format!("permalink parse error: {e}"))?;
    Ok(body.permalink)
}

/// Step 2: Publish the container. Returns the Instagram media ID.
/// Used for both single-image and carousel parent containers — the Graph API
/// endpoint is the same.
async fn publish_ig_container(
    ig_user_id: &str,
    container_id: &str,
    access_token: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct PublishResponse {
        id: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/{ig_user_id}/media_publish", ig_api_base()))
        .form(&[
            ("creation_id", container_id),
            ("access_token", access_token),
        ])
        .send()
        .await
        .map_err(|e| format!("Instagram publish network error: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Instagram publish failed: {body}"));
    }

    let r: PublishResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse publish response: {e}"))?;
    Ok(r.id)
}

// ── Tauri commands ────────────────────────────────────────────────────────

/// Resolved uploader configuration. Captured once per publish call so we don't
/// hit settings_db for every slide of a 5-image carousel.
/// Orchestrate the Instagram publish flow given pre-uploaded image URLs.
/// One image → single-image container + publish. Two or more → one carousel-item
/// container per image + a parent CAROUSEL container + publish. Extracted from
/// `publish_post` so integration tests can exercise it without an `AppState`,
/// access token plumbing, or real image hosting.
async fn publish_ig_flow(
    ig_user_id: &str,
    image_urls: &[String],
    caption: &str,
    access_token: &str,
) -> Result<String, String> {
    if image_urls.is_empty() {
        return Err("publish_ig_flow requires at least one image URL".to_string());
    }
    if image_urls.len() == 1 {
        let container_id =
            create_ig_container(ig_user_id, &image_urls[0], caption, access_token).await?;
        publish_ig_container(ig_user_id, &container_id, access_token).await
    } else {
        let mut children_ids: Vec<String> = Vec::with_capacity(image_urls.len());
        for url in image_urls {
            let id = create_ig_carousel_item(ig_user_id, url, access_token).await?;
            children_ids.push(id);
        }
        let parent_id =
            create_ig_carousel_parent(ig_user_id, &children_ids, caption, access_token).await?;
        publish_ig_container(ig_user_id, &parent_id, access_token).await
    }
}

enum ImageUploader {
    /// User configured an imgbb key — direct upload, faster & more reliable.
    Imgbb(String),
    /// Default: round-robin through Catbox → 0x0.st → tmpfiles.org. Litterbox
    /// is excluded because Meta's CDN refuses `litter.catbox.moe` URLs.
    Free,
}

impl ImageUploader {
    async fn from_state(state: &AppState) -> Result<Self, String> {
        let host = crate::db::settings_db::get(&state.db, "image_host")
            .await
            .unwrap_or_else(|| "catbox".to_string());
        if host == "imgbb" {
            let key = crate::db::settings_db::get(&state.db, "imgbb_api_key")
                .await
                .ok_or("Clé imgbb non configurée. Ajoute-la dans Paramètres → Publication.")?;
            Ok(Self::Imgbb(key))
        } else {
            Ok(Self::Free)
        }
    }

    async fn upload(&self, source: &str) -> Result<String, String> {
        match self {
            Self::Imgbb(key) => upload_image_to_imgbb(source, key).await,
            Self::Free => upload_image_free(source).await,
        }
    }
}

/// Resolve the connected account this post targets, with fallback for legacy
/// rows that pre-date migration 013.
///
/// Behavior:
/// 1. If `post.account_id` is set, fetch that account from DB. Validate that
///    its `provider` matches `expected_provider` (e.g. "instagram") so a draft
///    cross-posted to a wrong network is caught with a clear error rather
///    than silently publishing on the wrong platform.
/// 2. If `post.account_id` is None (legacy row), fall back to the first
///    connected account on the requested network. This keeps existing drafts
///    publishable but logs a warning so the issue surfaces.
async fn resolve_post_account(
    state: &AppState,
    post: &crate::db::history::PostRecord,
    expected_provider: &str,
) -> Result<crate::db::accounts::Account, String> {
    if let Some(aid) = post.account_id {
        let account = crate::db::accounts::get_by_id(&state.db, aid)
            .await
            .map_err(|e| {
                format!(
                    "Account {aid} attached to post {} could not be loaded: {e}. \
                 Has the account been disconnected?",
                    post.id
                )
            })?;
        if account.provider != expected_provider {
            return Err(format!(
                "Post {} is attached to a {} account but this publish flow expects {}.",
                post.id, account.provider, expected_provider
            ));
        }
        return Ok(account);
    }

    // Legacy fallback for posts created before migration 013.
    log::warn!(
        "publisher: post {} has no account_id (legacy row) — falling back to first \
         {expected_provider} account. This will publish under the wrong account if \
         multiple {expected_provider} accounts are connected.",
        post.id
    );
    let accounts = crate::db::accounts::list(&state.db).await?;
    accounts
        .into_iter()
        .find(|a| a.provider == expected_provider)
        .ok_or_else(|| {
            format!(
                "No {expected_provider} account connected. Connect one in \
                 Settings → Comptes."
            )
        })
}

/// Build the full caption (post body + " " + "#tag1 #tag2 …") used by every
/// publish flow. Hashtag-free if there are none.
fn compose_caption_with_hashtags(caption: &str, hashtags: &[String]) -> String {
    let hashtags_str = hashtags
        .iter()
        .map(|h| format!("#{h}"))
        .collect::<Vec<_>>()
        .join(" ");
    if hashtags_str.is_empty() {
        caption.to_string()
    } else {
        format!("{caption}\n\n{hashtags_str}")
    }
}

/// Publish a draft post to Instagram.
/// Auto-detects single-image vs carousel from `post.images.len()`.
#[tauri::command]
pub async fn publish_post(
    post_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<PublishResult, String> {
    // 1. Load the draft
    let post = crate::db::history::get_by_id(&state.db, post_id).await?;

    // Defensive: refuse if either the status flipped to "published" OR a
    // network media id is already attached. The status check is the happy
    // path; the ig_media_id check catches the out-of-sync case where a
    // prior publish completed on Meta's side but the local DB write was
    // interrupted (crash mid-update, network drop, etc.). Without this
    // second guard the user could double-publish to Instagram.
    if post.status == "published" || post.ig_media_id.is_some() {
        return Err(
            "Ce post est déjà publié. Ouvre-le depuis l'historique et clique sur \
             « Voir sur Instagram » pour le retrouver."
                .to_string(),
        );
    }
    if post.images.is_empty() {
        return Err("No image attached to this post. Generate an image first.".to_string());
    }

    // 2. Resolve the account this draft was generated for. The post stores
    //    `account_id` since migration 013 — using it ensures we publish via
    //    the same credentials the user picked when drafting, even if they
    //    have multiple Instagram accounts connected (was a silent bug).
    let account = resolve_post_account(&state, &post, "instagram").await?;

    // 3. Get access token (never leaves Rust)
    let access_token = crate::token_store::get_token(&account.token_key)?;

    // 4. Upload every image up-front so we have public URLs for the IG containers.
    //    imgbb if configured, otherwise free-tier auto-fallback chain.
    let uploader = ImageUploader::from_state(&state).await?;
    let mut image_urls: Vec<String> = Vec::with_capacity(post.images.len());
    for source in &post.images {
        let url = uploader.upload(source).await.map_err(|e| {
            format!(
                "Image upload failed for slide {} of {}: {e}",
                image_urls.len() + 1,
                post.images.len()
            )
        })?;
        image_urls.push(url);
    }

    // 5. Build caption with hashtags (used for single OR for carousel parent).
    let full_caption = compose_caption_with_hashtags(&post.caption, &post.hashtags);

    // 6. Branch + publish via the testable orchestrator.
    let media_id =
        publish_ig_flow(&account.user_id, &image_urls, &full_caption, &access_token).await?;

    // 7. Update post status in SQLite
    let published_at = Utc::now().to_rfc3339();
    crate::db::history::update_status(
        &state.db,
        post_id,
        "published",
        Some(&published_at),
        Some(&media_id),
    )
    .await?;

    // 8. Fetch the public permalink so the "Voir sur Instagram" button can
    //    deep-link to the actual /p/{shortcode}/ URL instead of the account
    //    profile feed. Best-effort — a failure here doesn't roll back the
    //    publish; the frontend falls back to the profile URL when this
    //    column stays NULL.
    if let Ok(permalink) = fetch_ig_permalink(&media_id, &access_token).await {
        let _ = crate::db::history::update_published_url(&state.db, post_id, &permalink).await;
    } else {
        log::warn!(
            "publisher: IG permalink fetch failed for media {media_id} — \
             frontend will fall back to profile feed URL"
        );
    }

    Ok(PublishResult {
        post_id,
        media_id,
        published_at,
    })
}

/// Save the imgbb API key in settings.
#[tauri::command]
pub async fn save_imgbb_key(
    api_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    crate::db::settings_db::set(&state.db, "imgbb_api_key", &api_key).await
}

/// Check if an imgbb API key is configured.
#[tauri::command]
pub async fn get_imgbb_key_status(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(crate::db::settings_db::get(&state.db, "imgbb_api_key")
        .await
        .is_some())
}

/// Save the image host provider ("catbox" | "imgbb").
#[tauri::command]
pub async fn save_image_host(
    provider: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if provider != "catbox" && provider != "imgbb" {
        return Err(format!("Invalid image host provider: {provider}"));
    }
    crate::db::settings_db::set(&state.db, "image_host", &provider).await
}

/// Get the configured image host provider (defaults to "catbox" if not set).
#[tauri::command]
pub async fn get_image_host(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(crate::db::settings_db::get(&state.db, "image_host")
        .await
        .unwrap_or_else(|| "catbox".to_string()))
}

/// Store a single image (base64 data URL or file path) on the draft so publish
/// commands can find it. Sets both `image_path` (legacy) and `images = [path]`
/// (new column) so the publish flow always sees consistent data.
#[tauri::command]
pub async fn update_draft_image(
    post_id: i64,
    image_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    crate::db::history::update_image_path(&state.db, post_id, &image_path).await
}

/// Store an array of images (carousel slides) on the draft. The order is
/// preserved as-is and used at publish time (slide 1 = images[0]).
/// `image_path` is also updated to `images[0]` for backward-compat readers.
#[tauri::command]
pub async fn update_draft_images(
    post_id: i64,
    images: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    crate::db::history::update_images(&state.db, post_id, &images).await
}

// ── LinkedIn publisher ────────────────────────────────────────────────────

/// Publish a draft post to LinkedIn (text-only or with image).
///
/// LinkedIn specifics:
///   - No imgbb needed — image uploaded directly as binary via registerUpload → PUT
///   - Hashtags embedded in the post text (LinkedIn has no separate hashtag field, max 5)
///   - Image format: PNG or JPEG, max 10 MB
///   - Author URN built from account.user_id (the LinkedIn profile ID)
#[tauri::command]
pub async fn publish_linkedin_post(
    post_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<PublishResult, String> {
    // 1. Load the draft
    let post = crate::db::history::get_by_id(&state.db, post_id).await?;

    // Same defensive guard as publish_post — see the comment there. ig_media_id
    // doubles as the LinkedIn URN, so a non-null value means a publish
    // completed on LinkedIn even if the local status update missed.
    if post.status == "published" || post.ig_media_id.is_some() {
        return Err(
            "Ce post est déjà publié. Ouvre-le depuis l'historique et clique sur \
             « Voir sur LinkedIn » pour le retrouver."
                .to_string(),
        );
    }

    // 2. Resolve the account this draft targets — see `resolve_post_account`.
    let account = resolve_post_account(&state, &post, "linkedin").await?;

    // 3. Access token (never leaves Rust)
    let access_token = crate::token_store::get_token(&account.token_key)?;

    // 4. Build text — hashtags embedded, capped at 5 per LinkedIn rules.
    // Sanitize each tag: keep only alphanumeric + underscore to prevent control-char injection.
    let hashtags_str = post
        .hashtags
        .iter()
        .take(5)
        .map(|h| {
            let clean: String = h
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            format!("#{clean}")
        })
        .filter(|t| t.len() > 1) // skip tags that became empty after sanitization
        .collect::<Vec<_>>()
        .join(" ");
    let full_text = if hashtags_str.is_empty() {
        post.caption.clone()
    } else {
        format!("{}\n\n{}", post.caption, hashtags_str)
    };

    // 5. Publish — with or without image(s)
    const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024; // LinkedIn hard limit per image: 10 MB

    let post_urn = if post.images.is_empty() {
        // Text-only post
        crate::adapters::linkedin::publish_text(&account.user_id, &full_text, &access_token).await?
    } else {
        // 1+ image(s): register + upload each, then a single publish_image with all asset URNs.
        // LinkedIn renders 2+ as a tiled gallery in the same post.
        let mut asset_urns: Vec<String> = Vec::with_capacity(post.images.len());
        for (idx, image_source) in post.images.iter().enumerate() {
            // Decode image bytes from data URL or file path, enforcing the 10 MB limit.
            let image_bytes = if let Some(b64) = image_source.strip_prefix("data:image/png;base64,")
            {
                if b64.len() * 3 / 4 > MAX_IMAGE_BYTES {
                    return Err(format!(
                        "Image {} of {} exceeds LinkedIn 10 MB limit",
                        idx + 1,
                        post.images.len()
                    ));
                }
                STANDARD
                    .decode(b64)
                    .map_err(|e| format!("Failed to decode base64 image {}: {e}", idx + 1))?
            } else {
                let meta = std::fs::metadata(image_source)
                    .map_err(|e| format!("Cannot stat image file '{image_source}': {e}"))?;
                if meta.len() > MAX_IMAGE_BYTES as u64 {
                    return Err(format!(
                        "Image {} of {} exceeds LinkedIn 10 MB limit",
                        idx + 1,
                        post.images.len()
                    ));
                }
                std::fs::read(image_source)
                    .map_err(|e| format!("Cannot read image file '{image_source}': {e}"))?
            };

            let (upload_url, asset_urn) =
                crate::adapters::linkedin::register_image_upload(&account.user_id, &access_token)
                    .await
                    .map_err(|e| format!("LinkedIn register upload (image {}): {e}", idx + 1))?;
            crate::adapters::linkedin::upload_image_binary(
                &upload_url,
                &image_bytes,
                &access_token,
            )
            .await
            .map_err(|e| format!("LinkedIn binary upload (image {}): {e}", idx + 1))?;
            asset_urns.push(asset_urn);
        }

        let urn_refs: Vec<&str> = asset_urns.iter().map(String::as_str).collect();
        crate::adapters::linkedin::publish_image(
            &account.user_id,
            &full_text,
            &urn_refs,
            &access_token,
        )
        .await?
    };

    // 6. Update post status in SQLite
    let published_at = Utc::now().to_rfc3339();
    crate::db::history::update_status(
        &state.db,
        post_id,
        "published",
        Some(&published_at),
        Some(&post_urn),
    )
    .await?;

    // 7. Store the public deep-link URL. LinkedIn URNs build their post URL
    //    deterministically (no API round-trip needed), so we can populate
    //    this column in-flight unlike Instagram which needs a follow-up
    //    Graph API call.
    let public_url = format!(
        "https://www.linkedin.com/feed/update/{}/",
        urlencoding::encode(&post_urn)
    );
    let _ = crate::db::history::update_published_url(&state.db, post_id, &public_url).await;

    Ok(PublishResult {
        post_id,
        media_id: post_urn,
        published_at,
    })
}

#[cfg(test)]
mod tests {
    //! Integration tests against a `wiremock` mock Graph API.
    //!
    //! The production code is unaware of the mock — it just queries the URL
    //! resolved by `ig_api_base()`, which we override via the env var
    //! `GETPOSTCRAFT_IG_API` for the duration of each test. `serial_test`
    //! ensures two tests don't collide on that single global.
    //!
    //! These tests would have caught the carousel publish regression PR #11
    //! fixed: the flow used to call `create_ig_container` with a single URL
    //! and ignore the rest, silently degrading carousels to single-image
    //! posts. The `expect(N)` assertions on each mock now make that visible.

    use super::*;
    use serial_test::serial;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const IG_USER: &str = "12345";
    const TOKEN: &str = "test-token";

    /// SAFETY: `#[serial]` on every test calling this guarantees no two
    /// tests touch the env var concurrently, so the unsafe write is sound.
    async fn boot_ig_mock() -> MockServer {
        let server = MockServer::start().await;
        unsafe {
            std::env::set_var("GETPOSTCRAFT_IG_API", server.uri());
        }
        server
    }

    fn clear_ig_env() {
        unsafe {
            std::env::remove_var("GETPOSTCRAFT_IG_API");
        }
    }

    #[tokio::test]
    #[serial]
    async fn publish_ig_flow_single_image_calls_one_container_then_publish() {
        let server = boot_ig_mock().await;

        // Single-image container — body has image_url + caption, NO is_carousel_item.
        Mock::given(method("POST"))
            .and(path(format!("/{IG_USER}/media")))
            .and(body_string_contains("image_url=https"))
            .and(body_string_contains("caption=Hello"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "container_single_1"
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path(format!("/{IG_USER}/media_publish")))
            .and(body_string_contains("creation_id=container_single_1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "ig_media_99"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let urls = vec!["https://example.com/image.png".to_string()];
        let media_id = publish_ig_flow(IG_USER, &urls, "Hello world", TOKEN)
            .await
            .expect("single-image publish should succeed");
        assert_eq!(media_id, "ig_media_99");

        drop(server);
        clear_ig_env();
    }

    #[tokio::test]
    #[serial]
    async fn publish_ig_flow_carousel_creates_items_then_parent_then_publish() {
        let server = boot_ig_mock().await;

        // Each carousel-item call returns a deterministic id keyed off the slide URL.
        // We assert that the parent gets called with all 3 ids in order.
        let item_ids = ["item_a", "item_b", "item_c"];
        for id in item_ids {
            Mock::given(method("POST"))
                .and(path(format!("/{IG_USER}/media")))
                .and(body_string_contains("is_carousel_item=true"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": id
                })))
                .up_to_n_times(1)
                .expect(1)
                .mount(&server)
                .await;
        }

        // Parent CAROUSEL container.
        Mock::given(method("POST"))
            .and(path(format!("/{IG_USER}/media")))
            .and(body_string_contains("media_type=CAROUSEL"))
            .and(body_string_contains("children=item_a%2Citem_b%2Citem_c"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "parent_xyz"
            })))
            .expect(1)
            .mount(&server)
            .await;

        // Final publish.
        Mock::given(method("POST"))
            .and(path(format!("/{IG_USER}/media_publish")))
            .and(body_string_contains("creation_id=parent_xyz"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "ig_media_carousel"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let urls: Vec<String> = (1..=3)
            .map(|i| format!("https://example.com/slide-{i}.png"))
            .collect();
        let media_id = publish_ig_flow(IG_USER, &urls, "Carousel caption", TOKEN)
            .await
            .expect("carousel publish should succeed");
        assert_eq!(media_id, "ig_media_carousel");

        drop(server);
        clear_ig_env();
    }

    #[tokio::test]
    #[serial]
    async fn publish_ig_flow_propagates_500_at_container_creation() {
        let server = boot_ig_mock().await;

        Mock::given(method("POST"))
            .and(path(format!("/{IG_USER}/media")))
            .respond_with(ResponseTemplate::new(500).set_body_string("Graph API down"))
            .expect(1)
            .mount(&server)
            .await;

        let urls = vec!["https://example.com/image.png".to_string()];
        let result = publish_ig_flow(IG_USER, &urls, "Hello", TOKEN).await;

        assert!(result.is_err(), "500 from container API must propagate");
        let err = result.unwrap_err();
        assert!(
            err.contains("Graph API down") || err.contains("container creation"),
            "error should reference upstream cause, got: {err}"
        );

        drop(server);
        clear_ig_env();
    }

    #[tokio::test]
    #[serial]
    async fn publish_ig_flow_aborts_when_first_carousel_item_fails() {
        let server = boot_ig_mock().await;

        Mock::given(method("POST"))
            .and(path(format!("/{IG_USER}/media")))
            .and(body_string_contains("is_carousel_item=true"))
            .respond_with(ResponseTemplate::new(400).set_body_string("Invalid image"))
            .expect(1)
            .mount(&server)
            .await;

        // No parent or publish mocks — if the code reached them, wiremock
        // would 404 and the assertion below would still hold (the only error
        // here would never be the abort signal we expect).
        let urls: Vec<String> = (1..=3)
            .map(|i| format!("https://example.com/slide-{i}.png"))
            .collect();
        let result = publish_ig_flow(IG_USER, &urls, "Caption", TOKEN).await;
        assert!(
            result.is_err(),
            "carousel must abort when any item creation fails"
        );

        drop(server);
        clear_ig_env();
    }

    #[tokio::test]
    #[serial]
    async fn publish_ig_flow_rejects_empty_url_slice() {
        // No mock needed — the function fails fast before any HTTP call.
        let result = publish_ig_flow(IG_USER, &[], "x", TOKEN).await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("at least one image URL"),
            "should fail fast with a clear message"
        );
    }
}
