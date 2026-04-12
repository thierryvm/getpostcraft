use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

const IG_API: &str = "https://graph.instagram.com/v21.0";
const IMGBB_API: &str = "https://api.imgbb.com/1/upload";

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishResult {
    pub post_id: i64,
    /// Platform-specific media ID or post URN (Instagram media ID, LinkedIn ugcPost URN, etc.)
    pub media_id: String,
    pub published_at: String,
}

// ── Image upload (imgbb) ───────────────────────────────────────────────────

/// Upload an image to imgbb and return the public URL.
/// `image_source` can be either:
///   - a local file path (absolute)
///   - a base64 data URL (`data:image/png;base64,...`) — as returned by the render pipeline
async fn upload_image_to_imgbb(image_source: &str, api_key: &str) -> Result<String, String> {
    // If the render pipeline already gave us a base64 data URL, reuse it directly.
    // Otherwise fall back to reading from disk (future-proofing for saved files).
    let b64 = if let Some(b64_part) = image_source.strip_prefix("data:image/png;base64,") {
        b64_part.to_string()
    } else {
        let bytes = std::fs::read(image_source)
            .map_err(|e| format!("Cannot read image file '{image_source}': {e}"))?;
        STANDARD.encode(&bytes)
    };

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

/// Step 1: Create a media container.
/// Returns the container ID to use in the publish step.
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
        .post(format!("{IG_API}/{ig_user_id}/media"))
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

/// Step 2: Publish the container. Returns the Instagram media ID.
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
        .post(format!("{IG_API}/{ig_user_id}/media_publish"))
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

/// Publish a draft post to Instagram.
/// The post must have an image_path set (use render_post_image first).
#[tauri::command]
pub async fn publish_post(
    post_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<PublishResult, String> {
    // 1. Load the draft
    let post = crate::db::history::get_by_id(&state.db, post_id).await?;

    if post.status == "published" {
        return Err("This post is already published".to_string());
    }

    let image_path = post
        .image_path
        .as_deref()
        .ok_or("No image attached to this post. Generate an image first.")?;

    // 2. Get connected Instagram account
    let accounts = crate::db::accounts::list(&state.db).await?;
    let account = accounts
        .iter()
        .find(|a| a.provider == "instagram")
        .ok_or("No Instagram account connected. Connect one in Settings → Comptes.")?;

    // 3. Get access token (never leaves Rust)
    let access_token = crate::token_store::get_token(&account.token_key)?;

    // 4. Get imgbb API key
    let imgbb_key = crate::db::settings_db::get(&state.db, "imgbb_api_key")
        .await
        .ok_or("imgbb API key not configured. Add it in Settings → Publication.")?;

    // 5. Upload image to imgbb → get public URL
    let image_url = upload_image_to_imgbb(image_path, &imgbb_key).await?;

    // 6. Build caption with hashtags
    let hashtags_str = post
        .hashtags
        .iter()
        .map(|h| format!("#{h}"))
        .collect::<Vec<_>>()
        .join(" ");
    let full_caption = if hashtags_str.is_empty() {
        post.caption.clone()
    } else {
        format!("{}\n\n{}", post.caption, hashtags_str)
    };

    // 7. Create Instagram media container
    let container_id =
        create_ig_container(&account.user_id, &image_url, &full_caption, &access_token).await?;

    // 8. Publish the container
    let media_id = publish_ig_container(&account.user_id, &container_id, &access_token).await?;

    // 9. Update post status in SQLite
    let published_at = Utc::now().to_rfc3339();
    crate::db::history::update_status(
        &state.db,
        post_id,
        "published",
        Some(&published_at),
        Some(&media_id),
    )
    .await?;

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

/// Store an image (base64 data URL or file path) on the draft so publish commands can find it.
#[tauri::command]
pub async fn update_draft_image(
    post_id: i64,
    image_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    crate::db::history::update_image_path(&state.db, post_id, &image_path).await
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

    if post.status == "published" {
        return Err("This post is already published".to_string());
    }

    // 2. Get connected LinkedIn account
    let accounts = crate::db::accounts::list(&state.db).await?;
    let account = accounts
        .iter()
        .find(|a| a.provider == "linkedin")
        .ok_or("No LinkedIn account connected. Connect one in Settings → Comptes.")?;

    // 3. Access token (never leaves Rust)
    let access_token = crate::token_store::get_token(&account.token_key)?;

    // 4. Build text — hashtags embedded, capped at 5 per LinkedIn best practices
    let hashtags_str = post
        .hashtags
        .iter()
        .take(5)
        .map(|h| format!("#{h}"))
        .collect::<Vec<_>>()
        .join(" ");
    let full_text = if hashtags_str.is_empty() {
        post.caption.clone()
    } else {
        format!("{}\n\n{}", post.caption, hashtags_str)
    };

    // 5. Publish — with or without image
    let post_urn = if let Some(image_source) = post.image_path.as_deref() {
        // Decode image bytes from data URL or file path
        let image_bytes = if let Some(b64) = image_source.strip_prefix("data:image/png;base64,") {
            STANDARD
                .decode(b64)
                .map_err(|e| format!("Failed to decode base64 image: {e}"))?
        } else {
            std::fs::read(image_source)
                .map_err(|e| format!("Cannot read image file '{image_source}': {e}"))?
        };

        // Step 1: Register upload → upload URL + asset URN
        let (upload_url, asset_urn) =
            crate::adapters::linkedin::register_image_upload(&account.user_id, &access_token)
                .await?;

        // Step 2: Upload binary
        crate::adapters::linkedin::upload_image_binary(&upload_url, &image_bytes, &access_token)
            .await?;

        // Step 3: Publish ugcPost with image
        crate::adapters::linkedin::publish_image(
            &account.user_id,
            &full_text,
            &asset_urn,
            &access_token,
        )
        .await?
    } else {
        // Text-only post
        crate::adapters::linkedin::publish_text(&account.user_id, &full_text, &access_token).await?
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

    Ok(PublishResult {
        post_id,
        media_id: post_urn,
        published_at,
    })
}
