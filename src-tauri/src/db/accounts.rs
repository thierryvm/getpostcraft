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
}

const SELECT_COLUMNS: &str = "id, provider, user_id, username, display_name, token_key, \
     created_at, updated_at, product_truth, brand_color, accent_color";

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
    }
}

pub async fn upsert_and_get(
    pool: &SqlitePool,
    provider: &str,
    user_id: &str,
    username: &str,
    display_name: Option<&str>,
    token_key: &str,
) -> Result<Account, String> {
    let sql = format!(
        "INSERT INTO accounts (provider, user_id, username, display_name, token_key, updated_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(provider, user_id) DO UPDATE SET
             username     = excluded.username,
             display_name = excluded.display_name,
             token_key    = excluded.token_key,
             updated_at   = excluded.updated_at
         RETURNING {SELECT_COLUMNS}",
    );
    let row = sqlx::query(&sql)
        .bind(provider)
        .bind(user_id)
        .bind(username)
        .bind(display_name)
        .bind(token_key)
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

pub async fn delete(pool: &SqlitePool, provider: &str, user_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM accounts WHERE provider = ? AND user_id = ?")
        .bind(provider)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
