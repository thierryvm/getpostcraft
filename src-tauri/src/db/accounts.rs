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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Fresh in-memory pool with all migrations applied — same pattern as
    /// `db::history::tests` and `db::groups::tests`. Each test gets its
    /// own isolated DB because `:memory:` is connection-private and we
    /// cap at `max_connections = 1`.
    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect("migrations apply cleanly");
        pool
    }

    async fn seed_account(pool: &SqlitePool, provider: &str, user_id: &str) -> Account {
        upsert_and_get(
            pool,
            provider,
            user_id,
            &format!("user_{user_id}"),
            Some(&format!("Display {user_id}")),
            &format!("{provider}:{user_id}"),
            Some("2099-12-31T23:59:59Z"),
        )
        .await
        .expect("seed upsert")
    }

    #[tokio::test]
    async fn upsert_inserts_new_account_and_returns_it() {
        // First call with a brand-new (provider, user_id) tuple must
        // INSERT and return the freshly-created row including the
        // server-side id allocation.
        let pool = fresh_pool().await;
        let acc = seed_account(&pool, "instagram", "12345").await;

        assert_eq!(acc.provider, "instagram");
        assert_eq!(acc.user_id, "12345");
        assert_eq!(acc.username, "user_12345");
        assert_eq!(acc.display_name.as_deref(), Some("Display 12345"));
        assert_eq!(acc.token_key, "instagram:12345");
        assert_eq!(
            acc.token_expires_at.as_deref(),
            Some("2099-12-31T23:59:59Z")
        );
        // Nullable columns left blank on first insert.
        assert!(acc.product_truth.is_none());
        assert!(acc.brand_color.is_none());
        assert!(acc.visual_profile.is_none());
        assert!(acc.display_handle.is_none());
    }

    #[tokio::test]
    async fn upsert_on_existing_account_updates_in_place() {
        // The ON CONFLICT(provider, user_id) DO UPDATE branch must
        // refresh mutable fields without changing the id. This is the
        // OAuth re-connection flow — same Instagram account, new token,
        // possibly new display name.
        let pool = fresh_pool().await;
        let first = seed_account(&pool, "instagram", "12345").await;

        let updated = upsert_and_get(
            &pool,
            "instagram",
            "12345",
            "new_username",
            Some("New Display"),
            "instagram:12345_new_token_key",
            Some("2100-01-01T00:00:00Z"),
        )
        .await
        .expect("second upsert");

        // Same row (no id rotation), refreshed fields.
        assert_eq!(updated.id, first.id);
        assert_eq!(updated.username, "new_username");
        assert_eq!(updated.display_name.as_deref(), Some("New Display"));
        assert_eq!(updated.token_key, "instagram:12345_new_token_key");
        assert_eq!(
            updated.token_expires_at.as_deref(),
            Some("2100-01-01T00:00:00Z")
        );
    }

    #[tokio::test]
    async fn list_returns_all_accounts_ordered_by_created_at() {
        let pool = fresh_pool().await;
        let a = seed_account(&pool, "instagram", "111").await;
        let b = seed_account(&pool, "linkedin", "222").await;
        let c = seed_account(&pool, "instagram", "333").await;

        let all = list(&pool).await.expect("list");
        assert_eq!(all.len(), 3);
        // Insertion order = creation order = list order.
        assert_eq!(all[0].id, a.id);
        assert_eq!(all[1].id, b.id);
        assert_eq!(all[2].id, c.id);
    }

    #[tokio::test]
    async fn get_by_id_returns_account_and_404s_on_missing() {
        let pool = fresh_pool().await;
        let acc = seed_account(&pool, "instagram", "12345").await;

        let fetched = get_by_id(&pool, acc.id).await.expect("get_by_id hit");
        assert_eq!(fetched.id, acc.id);
        assert_eq!(fetched.user_id, "12345");

        let err = get_by_id(&pool, 99999)
            .await
            .expect_err("missing id must return Err");
        assert!(
            err.contains("99999"),
            "error should surface the id queried, got: {err}"
        );
    }

    #[tokio::test]
    async fn update_product_truth_round_trips_and_clears() {
        // SECURITY-ADJACENT: product_truth is content the AI prompts
        // get conditioned on. A failed set or a silent revert would
        // affect every future generation for that account. Round-trip
        // verify both set + clear.
        let pool = fresh_pool().await;
        let acc = seed_account(&pool, "instagram", "12345").await;

        update_product_truth(&pool, acc.id, Some("Brand X: minimalist DevOps"))
            .await
            .expect("set");
        let after_set = get_by_id(&pool, acc.id).await.expect("get");
        assert_eq!(
            after_set.product_truth.as_deref(),
            Some("Brand X: minimalist DevOps")
        );

        update_product_truth(&pool, acc.id, None)
            .await
            .expect("clear");
        let after_clear = get_by_id(&pool, acc.id).await.expect("get");
        assert!(after_clear.product_truth.is_none());
    }

    #[tokio::test]
    async fn update_branding_sets_both_colors_atomically() {
        let pool = fresh_pool().await;
        let acc = seed_account(&pool, "instagram", "12345").await;

        update_branding(&pool, acc.id, Some("#0d1117"), Some("#3ddc84"))
            .await
            .expect("set both");
        let after = get_by_id(&pool, acc.id).await.expect("get");
        assert_eq!(after.brand_color.as_deref(), Some("#0d1117"));
        assert_eq!(after.accent_color.as_deref(), Some("#3ddc84"));

        // Clearing one but keeping the other — both Option<&str> are
        // bound independently, so a single-clear is valid.
        update_branding(&pool, acc.id, None, Some("#ff6b6b"))
            .await
            .expect("partial clear");
        let after_partial = get_by_id(&pool, acc.id).await.expect("get");
        assert!(after_partial.brand_color.is_none());
        assert_eq!(after_partial.accent_color.as_deref(), Some("#ff6b6b"));
    }

    #[tokio::test]
    async fn update_display_handle_overrides_and_clears() {
        // Regression guard for migration 016: LinkedIn accounts get
        // `username` populated with the owner's full personal name. The
        // brand stamp on rendered visuals needs a separate handle.
        // None clears, falling back to `username` at render time.
        let pool = fresh_pool().await;
        let acc = seed_account(&pool, "linkedin", "abc-xyz").await;

        update_display_handle(&pool, acc.id, Some("terminallearning"))
            .await
            .expect("set");
        let after_set = get_by_id(&pool, acc.id).await.expect("get");
        assert_eq!(
            after_set.display_handle.as_deref(),
            Some("terminallearning")
        );

        update_display_handle(&pool, acc.id, None)
            .await
            .expect("clear");
        let after_clear = get_by_id(&pool, acc.id).await.expect("get");
        assert!(after_clear.display_handle.is_none());
    }

    #[tokio::test]
    async fn update_visual_profile_round_trips_and_clears() {
        let pool = fresh_pool().await;
        let acc = seed_account(&pool, "instagram", "12345").await;

        let profile_json = r##"{"colors":["#0d1117","#3ddc84"],"mood":["tech"]}"##;
        update_visual_profile(&pool, acc.id, Some(profile_json))
            .await
            .expect("set");
        let after = get_by_id(&pool, acc.id).await.expect("get");
        assert_eq!(after.visual_profile.as_deref(), Some(profile_json));

        update_visual_profile(&pool, acc.id, None)
            .await
            .expect("clear");
        let after_clear = get_by_id(&pool, acc.id).await.expect("get");
        assert!(after_clear.visual_profile.is_none());
    }

    #[tokio::test]
    async fn delete_removes_account_and_nulls_post_references() {
        // SECURITY-ADJACENT: deleting an account must not leave
        // dangling foreign keys in post_history. SQLite can't enforce
        // ON DELETE SET NULL via ALTER TABLE ADD COLUMN (migration 013
        // hotfix), so the cascade lives in this function. Verify it
        // actually fires.
        let pool = fresh_pool().await;
        let acc = seed_account(&pool, "instagram", "12345").await;

        // Insert a post pinned to this account.
        sqlx::query(
            "INSERT INTO post_history (network, caption, hashtags, status, created_at, \
             images, account_id) VALUES ('instagram', 'test', '[]', 'draft', \
             '2026-01-01T00:00:00Z', '[]', ?)",
        )
        .bind(acc.id)
        .execute(&pool)
        .await
        .expect("insert post");

        delete(&pool, "instagram", "12345").await.expect("delete");

        // Account gone.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(count, 0);

        // Post still there but unpinned.
        let post_account_id: Option<i64> =
            sqlx::query_scalar("SELECT account_id FROM post_history WHERE caption = 'test'")
                .fetch_one(&pool)
                .await
                .expect("get post");
        assert!(
            post_account_id.is_none(),
            "post_history.account_id must be NULL after the source account is deleted"
        );
    }

    #[tokio::test]
    async fn delete_is_idempotent_on_missing_account() {
        // Calling delete for an account that doesn't exist must not
        // error — the publish flow may race with a user clicking
        // disconnect, and we'd rather no-op than panic.
        let pool = fresh_pool().await;
        delete(&pool, "instagram", "ghost-id")
            .await
            .expect("delete on missing must be Ok");
    }
}
