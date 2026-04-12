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
    pub ig_media_id: String,
    pub published_at: String,
}

// ── Image upload (imgbb) ───────────────────────────────────────────────────

/// Upload a local PNG to imgbb and return the public URL.
/// Requires an imgbb API key stored in settings as "imgbb_api_key".
async fn upload_image_to_imgbb(image_path: &str, api_key: &str) -> Result<String, String> {
    let bytes = std::fs::read(image_path)
        .map_err(|e| format!("Cannot read image file {image_path}: {e}"))?;
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
    let ig_media_id = publish_ig_container(&account.user_id, &container_id, &access_token).await?;

    // 9. Update post status in SQLite
    let published_at = Utc::now().to_rfc3339();
    crate::db::history::update_status(
        &state.db,
        post_id,
        "published",
        Some(&published_at),
        Some(&ig_media_id),
    )
    .await?;

    Ok(PublishResult {
        post_id,
        ig_media_id,
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
