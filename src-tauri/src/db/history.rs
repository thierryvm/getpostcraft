use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostRecord {
    pub id: i64,
    pub network: String,
    pub caption: String,
    pub hashtags: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub published_at: Option<String>,
    pub scheduled_at: Option<String>,
    /// Legacy single-image source (file path or base64 data URL).
    /// Kept for backward compat with single-image flows; equals `images[0]`
    /// after a migration backfill or any new save.
    pub image_path: Option<String>,
    /// All carousel images (or a single-image array). Empty for text-only posts.
    /// New writers populate this; readers pick single vs carousel based on len.
    pub images: Vec<String>,
    pub ig_media_id: Option<String>,
    /// Which connected account this post was generated for. The publish flow
    /// uses this to target the right credentials when the user has multiple
    /// accounts on the same network. NULL on legacy rows or generation flows
    /// that ran without an account selection (preview-only).
    pub account_id: Option<i64>,
}

pub async fn insert_draft(
    pool: &SqlitePool,
    network: &str,
    caption: &str,
    hashtags: &[String],
    image_path: Option<&str>,
    account_id: Option<i64>,
) -> Result<i64, String> {
    let hashtags_json = serde_json::to_string(hashtags).map_err(|e| e.to_string())?;
    let images_json = match image_path {
        Some(path) if !path.is_empty() => {
            serde_json::to_string(&vec![path.to_string()]).map_err(|e| e.to_string())?
        }
        _ => "[]".to_string(),
    };
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO post_history \
            (network, caption, hashtags, status, created_at, image_path, images, account_id)
         VALUES (?, ?, ?, 'draft', ?, ?, ?, ?)",
    )
    .bind(network)
    .bind(caption)
    .bind(&hashtags_json)
    .bind(&now)
    .bind(image_path)
    .bind(&images_json)
    .bind(account_id)
    .execute(pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<PostRecord, String> {
    let row = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, images, ig_media_id, account_id
         FROM post_history WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|_| format!("Post {id} not found"))?;
    row_to_post_record(&row)
}

pub async fn update_status(
    pool: &SqlitePool,
    id: i64,
    status: &str,
    published_at: Option<&str>,
    ig_media_id: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE post_history SET status = ?, published_at = ?, ig_media_id = ? WHERE id = ?",
    )
    .bind(status)
    .bind(published_at)
    .bind(ig_media_id)
    .bind(id)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub async fn list_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<PostRecord>, String> {
    let rows: Vec<SqliteRow> = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, images, ig_media_id, account_id
         FROM post_history ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    rows.iter().map(row_to_post_record).collect()
}

/// List posts whose scheduled_at falls within [from, to] (ISO-8601 date strings).
pub async fn list_in_range(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> Result<Vec<PostRecord>, String> {
    let rows: Vec<SqliteRow> = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, images, ig_media_id, account_id
         FROM post_history
         WHERE (scheduled_at IS NOT NULL AND scheduled_at BETWEEN ? AND ?)
            OR (scheduled_at IS NULL AND created_at BETWEEN ? AND ?)
         ORDER BY COALESCE(scheduled_at, created_at) ASC",
    )
    .bind(from)
    .bind(to)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    rows.iter().map(row_to_post_record).collect()
}

/// Attach a single image (or base64 data URL) to a post.
/// Sets both legacy `image_path` and the new `images` array (= [image_path]) so
/// readers on either column see the same source. For carousels, use `update_images`.
pub async fn update_image_path(pool: &SqlitePool, id: i64, image_path: &str) -> Result<(), String> {
    let images_json =
        serde_json::to_string(&vec![image_path.to_string()]).map_err(|e| e.to_string())?;
    sqlx::query("UPDATE post_history SET image_path = ?, images = ? WHERE id = ?")
        .bind(image_path)
        .bind(&images_json)
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Attach an array of images (carousel slides) to a post.
/// `image_path` is also set to `images[0]` for backward compatibility with any
/// reader that still consults the legacy column. An empty slice clears both.
pub async fn update_images(pool: &SqlitePool, id: i64, images: &[String]) -> Result<(), String> {
    let images_json = serde_json::to_string(images).map_err(|e| e.to_string())?;
    let first = images.first().map(String::as_str);
    sqlx::query("UPDATE post_history SET image_path = ?, images = ? WHERE id = ?")
        .bind(first)
        .bind(&images_json)
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Set (or clear) the scheduled_at date for a post.
pub async fn set_scheduled_at(
    pool: &SqlitePool,
    post_id: i64,
    scheduled_at: Option<&str>,
) -> Result<(), String> {
    sqlx::query("UPDATE post_history SET scheduled_at = ? WHERE id = ?")
        .bind(scheduled_at)
        .bind(post_id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Delete a post by id.
pub async fn delete_post(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM post_history WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Update caption and hashtags for a draft post.
/// Returns an error if the post is not in draft status.
pub async fn update_draft_content(
    pool: &SqlitePool,
    id: i64,
    caption: &str,
    hashtags: &[String],
) -> Result<(), String> {
    let hashtags_json = serde_json::to_string(hashtags).map_err(|e| e.to_string())?;
    let rows_affected = sqlx::query(
        "UPDATE post_history SET caption = ?, hashtags = ? WHERE id = ? AND status = 'draft'",
    )
    .bind(caption)
    .bind(&hashtags_json)
    .bind(id)
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .map_err(|e| e.to_string())?;

    if rows_affected == 0 {
        Err("Post not found or not a draft".to_string())
    } else {
        Ok(())
    }
}

/// Parse the `images` JSON column with a graceful fallback chain so we keep
/// working on rows the migration hasn't touched yet (e.g. fresh installs of
/// older builds opening newer DBs in dev).
fn parse_images_column(raw: Option<String>, image_path: Option<&str>) -> Vec<String> {
    if let Some(text) = raw.as_deref() {
        if !text.trim().is_empty() {
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(text) {
                return parsed;
            }
        }
    }
    match image_path {
        Some(p) if !p.is_empty() => vec![p.to_string()],
        _ => Vec::new(),
    }
}

fn row_to_post_record(r: &SqliteRow) -> Result<PostRecord, String> {
    let hashtags_str: String = r.try_get("hashtags").map_err(|e| e.to_string())?;
    let hashtags: Vec<String> = serde_json::from_str(&hashtags_str).unwrap_or_default();
    let image_path: Option<String> = r.try_get("image_path").map_err(|e| e.to_string())?;
    let images_raw: Option<String> = r.try_get("images").ok().flatten();
    let images = parse_images_column(images_raw, image_path.as_deref());
    Ok(PostRecord {
        id: r.try_get("id").map_err(|e| e.to_string())?,
        network: r.try_get("network").map_err(|e| e.to_string())?,
        caption: r.try_get("caption").map_err(|e| e.to_string())?,
        hashtags,
        status: r.try_get("status").map_err(|e| e.to_string())?,
        created_at: r.try_get("created_at").map_err(|e| e.to_string())?,
        published_at: r.try_get("published_at").map_err(|e| e.to_string())?,
        scheduled_at: r.try_get("scheduled_at").map_err(|e| e.to_string())?,
        image_path,
        images,
        ig_media_id: r.try_get("ig_media_id").map_err(|e| e.to_string())?,
        account_id: r.try_get("account_id").ok().flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_images_column_returns_array_when_json_valid() {
        let raw = Some(r#"["img1","img2","img3"]"#.to_string());
        let result = parse_images_column(raw, Some("legacy"));
        assert_eq!(result, vec!["img1", "img2", "img3"]);
    }

    #[test]
    fn parse_images_column_falls_back_to_image_path_when_null() {
        let result = parse_images_column(None, Some("legacy_path"));
        assert_eq!(result, vec!["legacy_path"]);
    }

    #[test]
    fn parse_images_column_falls_back_when_blank_or_invalid_json() {
        assert_eq!(
            parse_images_column(Some(String::new()), Some("p")),
            vec!["p"]
        );
        assert_eq!(
            parse_images_column(Some("   ".into()), Some("p")),
            vec!["p"]
        );
        assert_eq!(
            parse_images_column(Some("not json".into()), Some("p")),
            vec!["p"]
        );
    }

    #[test]
    fn parse_images_column_returns_empty_when_nothing_set() {
        assert!(parse_images_column(None, None).is_empty());
        assert!(parse_images_column(None, Some("")).is_empty());
    }

    #[test]
    fn parse_images_column_preserves_order_of_carousel_slides() {
        // Carousel publishing depends on slide order — slide 1 must come back as images[0].
        let raw = Some(r#"["slide-1","slide-2","slide-3","slide-4","slide-5"]"#.to_string());
        let result = parse_images_column(raw, None);
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], "slide-1");
        assert_eq!(result[4], "slide-5");
    }
}
