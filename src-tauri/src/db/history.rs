use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

#[derive(Debug, Serialize, Deserialize)]
pub struct PostRecord {
    pub id: i64,
    pub network: String,
    pub caption: String,
    pub hashtags: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub published_at: Option<String>,
    pub scheduled_at: Option<String>,
    pub image_path: Option<String>,
    pub ig_media_id: Option<String>,
}

pub async fn insert_draft(
    pool: &SqlitePool,
    network: &str,
    caption: &str,
    hashtags: &[String],
    image_path: Option<&str>,
) -> Result<i64, String> {
    let hashtags_json = serde_json::to_string(hashtags).map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO post_history (network, caption, hashtags, status, created_at, image_path)
         VALUES (?, ?, ?, 'draft', ?, ?)",
    )
    .bind(network)
    .bind(caption)
    .bind(&hashtags_json)
    .bind(&now)
    .bind(image_path)
    .execute(pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<PostRecord, String> {
    let row = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, ig_media_id
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
                scheduled_at, image_path, ig_media_id
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
                scheduled_at, image_path, ig_media_id
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

fn row_to_post_record(r: &SqliteRow) -> Result<PostRecord, String> {
    let hashtags_str: String = r.try_get("hashtags").map_err(|e| e.to_string())?;
    let hashtags: Vec<String> = serde_json::from_str(&hashtags_str).unwrap_or_default();
    Ok(PostRecord {
        id: r.try_get("id").map_err(|e| e.to_string())?,
        network: r.try_get("network").map_err(|e| e.to_string())?,
        caption: r.try_get("caption").map_err(|e| e.to_string())?,
        hashtags,
        status: r.try_get("status").map_err(|e| e.to_string())?,
        created_at: r.try_get("created_at").map_err(|e| e.to_string())?,
        published_at: r.try_get("published_at").map_err(|e| e.to_string())?,
        scheduled_at: r.try_get("scheduled_at").map_err(|e| e.to_string())?,
        image_path: r.try_get("image_path").map_err(|e| e.to_string())?,
        ig_media_id: r.try_get("ig_media_id").map_err(|e| e.to_string())?,
    })
}
