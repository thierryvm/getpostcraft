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
}

pub async fn insert_draft(
    pool: &SqlitePool,
    network: &str,
    caption: &str,
    hashtags: &[String],
) -> Result<i64, String> {
    let hashtags_json = serde_json::to_string(hashtags).map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO post_history (network, caption, hashtags, status, created_at)
         VALUES (?, ?, ?, 'draft', ?)",
    )
    .bind(network)
    .bind(caption)
    .bind(&hashtags_json)
    .bind(&now)
    .execute(pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

pub async fn list_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<PostRecord>, String> {
    let rows: Vec<SqliteRow> = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at
         FROM post_history ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    rows.iter()
        .map(|r| {
            let hashtags_str: String = r.try_get("hashtags").map_err(|e| e.to_string())?;
            let hashtags: Vec<String> =
                serde_json::from_str(&hashtags_str).unwrap_or_default();
            Ok(PostRecord {
                id: r.try_get("id").map_err(|e| e.to_string())?,
                network: r.try_get("network").map_err(|e| e.to_string())?,
                caption: r.try_get("caption").map_err(|e| e.to_string())?,
                hashtags,
                status: r.try_get("status").map_err(|e| e.to_string())?,
                created_at: r.try_get("created_at").map_err(|e| e.to_string())?,
                published_at: r.try_get("published_at").map_err(|e| e.to_string())?,
            })
        })
        .collect()
}
