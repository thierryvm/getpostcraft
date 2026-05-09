use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: i64,
    pub provider: String,
    pub user_id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub token_key: String,
    pub created_at: String,
    pub updated_at: String,
    pub product_truth: Option<String>,
    pub brand_color: Option<String>,
    pub accent_color: Option<String>,
    /// JSON blob produced by the Vision-based analyzer in PR-GPC-7.
    /// Shape: see migration 012. Read by the post generation flow as a
    /// secondary style hint on top of `product_truth`. None = no analysis run.
    pub visual_profile: Option<String>,
    /// ISO 8601 / RFC 3339 UTC timestamp when the OAuth access token in
    /// the keyring will expire. NULL on legacy rows (created before
    /// migration 014) or providers that don't return `expires_in` —
    /// the publish flow then falls back to the upstream-API behaviour
    /// (silent 401 from Meta/LinkedIn) until the user reconnects.
    pub token_expires_at: Option<String>,
    /// Brand handle to display on rendered visuals (e.g. `@terminallearning`).
    /// Nullable so the renderer can fall back to `username` for accounts
    /// that already have a handle-style username (Instagram). Use this
    /// for LinkedIn where OAuth fills `username` with the owner's full
    /// personal name. Preserved across re-connections by `upsert_and_get`.
    pub display_handle: Option<String>,
}

const SELECT_COLUMNS: &str = "id, provider, user_id, username, display_name, token_key, \
     created_at, updated_at, product_truth, brand_color, accent_color, visual_profile, \
     token_expires_at, display_handle";

fn row_to_account(row: &SqliteRow) -> Account {
    Account {
        id: row.get("id"),
        provider: row.get("provider"),
        user_id: row.get("user_id"),
        username: row.get("username"),
        display_name: row.get("display_name"),
        token_key: row.get("token_key"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        product_truth: row.get("product_truth"),
        brand_color: row.get("brand_color"),
        accent_color: row.get("accent_color"),
        visual_profile: row.try_get("visual_profile").ok().flatten(),
        token_expires_at: row.try_get("token_expires_at").ok().flatten(),
        display_handle: row.try_get("display_handle").ok().flatten(),
    }
}

pub async fn update_visual_profile(
    pool: &SqlitePool,
    id: i64,
    visual_profile: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE accounts SET visual_profile = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(visual_profile)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn upsert_and_get(
    pool: &SqlitePool,
    provider: &str,
    user_id: &str,
    username: &str,
    display_name: Option<&str>,
    token_key: &str,
    token_expires_at: Option<&str>,
) -> Result<Account, String> {
    let sql = format!(
        "INSERT INTO accounts (provider, user_id, username, display_name, token_key, \
                               token_expires_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(provider, user_id) DO UPDATE SET
             username         = excluded.username,
             display_name     = excluded.display_name,
             token_key        = excluded.token_key,
             token_expires_at = excluded.token_expires_at,
             updated_at       = excluded.updated_at
         RETURNING {SELECT_COLUMNS}",
    );
    let row = sqlx::query(&sql)
        .bind(provider)
        .bind(user_id)
        .bind(username)
        .bind(display_name)
        .bind(token_key)
        .bind(token_expires_at)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(row_to_account(&row))
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Account>, String> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM accounts ORDER BY created_at ASC");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows.iter().map(row_to_account).collect())
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Account, String> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM accounts WHERE id = ?");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Account {id} not found: {e}"))?;

    Ok(row_to_account(&row))
}

pub async fn update_product_truth(
    pool: &SqlitePool,
    id: i64,
    product_truth: Option<&str>,
) -> Result<(), String> {
    sqlx::query("UPDATE accounts SET product_truth = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(product_truth)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn update_branding(
    pool: &SqlitePool,
    id: i64,
    brand_color: Option<&str>,
    accent_color: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE accounts SET brand_color = ?, accent_color = ?, updated_at = datetime('now') \
         WHERE id = ?",
    )
    .bind(brand_color)
    .bind(accent_color)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Save or clear the brand handle to display on rendered visuals.
/// Pass `None` to clear (falls back to `username` at render time).
pub async fn update_display_handle(
    pool: &SqlitePool,
    id: i64,
    display_handle: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE accounts SET display_handle = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(display_handle)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, provider: &str, user_id: &str) -> Result<(), String> {
    // Find the account first so we can clear its id from any post that
    // references it. SQLite forbids FOREIGN KEY in ALTER TABLE ADD COLUMN
    // (see migration 013), so the cascade is enforced here in app code.
    let row = sqlx::query("SELECT id FROM accounts WHERE provider = ? AND user_id = ?")
        .bind(provider)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(r) = row {
        let account_id: i64 = r.get("id");
        sqlx::query("UPDATE post_history SET account_id = NULL WHERE account_id = ?")
            .bind(account_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    sqlx::query("DELETE FROM accounts WHERE provider = ? AND user_id = ?")
        .bind(provider)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
