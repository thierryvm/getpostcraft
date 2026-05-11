use sqlx::SqlitePool;

pub async fn get(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Fresh in-memory pool with all migrations applied. The `settings`
    /// table is created and seeded with default rows (active_provider,
    /// active_model) by the migration chain — see `migration_tests`.
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

    #[tokio::test]
    async fn get_returns_none_for_unknown_key() {
        let pool = fresh_pool().await;
        let v = get(&pool, "completely-unknown-key").await;
        assert!(v.is_none(), "unknown key must yield None, got: {v:?}");
    }

    #[tokio::test]
    async fn set_then_get_round_trips() {
        let pool = fresh_pool().await;
        set(&pool, "my-key", "my-value").await.expect("set");
        let v = get(&pool, "my-key").await;
        assert_eq!(v.as_deref(), Some("my-value"));
    }

    #[tokio::test]
    async fn set_on_existing_key_upserts_the_value() {
        // The ON CONFLICT(key) DO UPDATE branch must replace the old
        // value in place. Used at startup to refresh active_provider /
        // active_model when the user picks a new model in Settings.
        let pool = fresh_pool().await;
        set(&pool, "active_model", "anthropic/claude-haiku-latest")
            .await
            .expect("first set");
        set(&pool, "active_model", "anthropic/claude-sonnet-4.6")
            .await
            .expect("second set");
        let v = get(&pool, "active_model").await;
        assert_eq!(
            v.as_deref(),
            Some("anthropic/claude-sonnet-4.6"),
            "second set must override the first"
        );
    }

    #[tokio::test]
    async fn migrations_seed_default_provider_and_model() {
        // Smoke: the migration chain seeds default active_provider and
        // active_model so the first AI call doesn't crash on a fresh
        // install. Migrations 001 / 005 / 006 / 008 / 010 are jointly
        // responsible. Re-asserted here from the consumer's POV so a
        // future migration that breaks the seed gets caught.
        let pool = fresh_pool().await;
        let provider = get(&pool, "active_provider").await;
        let model = get(&pool, "active_model").await;
        assert!(
            provider.as_deref().is_some_and(|s| !s.trim().is_empty()),
            "active_provider must be seeded, got: {provider:?}"
        );
        assert!(
            model.as_deref().is_some_and(|s| !s.trim().is_empty()),
            "active_model must be seeded, got: {model:?}"
        );
    }

    #[tokio::test]
    async fn set_accepts_empty_string_value() {
        // The setter doesn't enforce non-empty values — that's a UI
        // concern. Allowing empty strings keeps the function a pure
        // key-value store and lets the UI clear settings by writing
        // "" rather than needing a separate delete op.
        let pool = fresh_pool().await;
        set(&pool, "my-key", "").await.expect("set empty");
        let v = get(&pool, "my-key").await;
        assert_eq!(v.as_deref(), Some(""));
    }
}
