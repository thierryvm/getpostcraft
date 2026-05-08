/// AI usage ledger — see migration 015 for the schema rationale (we store
/// tokens, compute cost at query time against `network_rules::price_for`).
use serde::Serialize;
use sqlx::{Row, SqlitePool};

/// Insert one usage record. Called from each AI command after a successful
/// sidecar call. Failure to record is logged but never propagated — we'd
/// rather miss a row in the ledger than fail the user-facing AI call over
/// a bookkeeping error.
pub async fn insert(
    pool: &SqlitePool,
    provider: &str,
    model: &str,
    action: &str,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO ai_usage (occurred_at, provider, model, action, input_tokens, output_tokens) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&now)
    .bind(provider)
    .bind(model)
    .bind(action)
    .bind(input_tokens)
    .bind(output_tokens)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Aggregated usage for the UI summary panel. Returned by
/// `get_ai_usage_summary` Tauri command.
#[derive(Debug, Serialize)]
pub struct UsageSummary {
    /// Calls in the last 30 days.
    pub calls_30d: i64,
    /// Cost in USD for the last 30 days, rounded to 4 decimals.
    pub cost_usd_30d: f64,
    /// Cost in USD month-to-date (current calendar month, UTC).
    pub cost_usd_month: f64,
    /// Per-model breakdown over the last 30 days, sorted by cost desc.
    pub by_model_30d: Vec<UsageByModel>,
}

#[derive(Debug, Serialize)]
pub struct UsageByModel {
    pub model: String,
    pub provider: String,
    pub calls: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    /// True when the pricing came from a fallback rather than a known
    /// model entry — the UI flags this so the user knows the figure is
    /// approximate.
    pub price_estimated: bool,
}

/// Build the summary from raw rows. Cost is computed here using the
/// pricing map in `network_rules` so a price update doesn't require a
/// data migration — the historical token counts stay valid.
pub async fn summarise(pool: &SqlitePool) -> Result<UsageSummary, String> {
    use chrono::{Datelike, Utc};

    let now = Utc::now();
    let thirty_days_ago = (now - chrono::Duration::days(30)).to_rfc3339();
    // ISO 8601 string sort works for the boundary check because RFC 3339
    // lexicographic order matches chronological order.
    let month_start = format!("{:04}-{:02}-01T00:00:00+00:00", now.year(), now.month());

    // Per-model rollup over the last 30 days.
    let rows = sqlx::query(
        "SELECT provider, model, COUNT(*) as calls, \
                COALESCE(SUM(input_tokens), 0) as input_tokens, \
                COALESCE(SUM(output_tokens), 0) as output_tokens \
         FROM ai_usage \
         WHERE occurred_at >= ? \
         GROUP BY provider, model \
         ORDER BY calls DESC",
    )
    .bind(&thirty_days_ago)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut by_model = Vec::with_capacity(rows.len());
    let mut total_calls_30d: i64 = 0;
    let mut total_cost_30d: f64 = 0.0;

    for row in rows {
        let provider: String = row.get("provider");
        let model: String = row.get("model");
        let calls: i64 = row.get("calls");
        let input_tokens: i64 = row.get("input_tokens");
        let output_tokens: i64 = row.get("output_tokens");

        let (price_in, price_out, estimated) = crate::network_rules::price_for(&model);
        // price_* are USD per million tokens; tokens / 1_000_000 * price_*
        let cost_usd = (input_tokens as f64 / 1_000_000.0) * price_in
            + (output_tokens as f64 / 1_000_000.0) * price_out;

        total_calls_30d += calls;
        total_cost_30d += cost_usd;

        by_model.push(UsageByModel {
            model,
            provider,
            calls,
            input_tokens,
            output_tokens,
            cost_usd: round_4(cost_usd),
            price_estimated: estimated,
        });
    }

    // Sort by cost descending so the heaviest spend lands first in the UI.
    by_model.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Month-to-date total.
    let month_rows = sqlx::query(
        "SELECT model, COALESCE(SUM(input_tokens), 0) as input_tokens, \
                COALESCE(SUM(output_tokens), 0) as output_tokens \
         FROM ai_usage \
         WHERE occurred_at >= ? \
         GROUP BY model",
    )
    .bind(&month_start)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut total_cost_month: f64 = 0.0;
    for row in month_rows {
        let model: String = row.get("model");
        let input_tokens: i64 = row.get("input_tokens");
        let output_tokens: i64 = row.get("output_tokens");
        let (price_in, price_out, _) = crate::network_rules::price_for(&model);
        total_cost_month += (input_tokens as f64 / 1_000_000.0) * price_in
            + (output_tokens as f64 / 1_000_000.0) * price_out;
    }

    Ok(UsageSummary {
        calls_30d: total_calls_30d,
        cost_usd_30d: round_4(total_cost_30d),
        cost_usd_month: round_4(total_cost_month),
        by_model_30d: by_model,
    })
}

/// Round to 4 decimals — USD cents are 2, plus an extra 2 to keep
/// micro-call costs visible (one Haiku call is ~$0.0001).
fn round_4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect("migrate");
        pool
    }

    #[tokio::test]
    async fn summarise_empty_db_returns_zero_with_no_models() {
        let pool = fresh_pool().await;
        let s = summarise(&pool).await.expect("summarise");
        assert_eq!(s.calls_30d, 0);
        assert_eq!(s.cost_usd_30d, 0.0);
        assert_eq!(s.cost_usd_month, 0.0);
        assert!(s.by_model_30d.is_empty());
    }

    #[tokio::test]
    async fn summarise_aggregates_calls_and_computes_cost_from_pricing_map() {
        let pool = fresh_pool().await;

        // Two calls on Sonnet (3.00 / 15.00 per million).
        // Tokens chosen to produce a clean, hand-checkable cost.
        // Total input = 1_000_000, output = 200_000 → cost = 3.00 + 3.00 = 6.00
        insert(
            &pool,
            "openrouter",
            "anthropic/claude-sonnet-4.6",
            "generate_content",
            600_000,
            100_000,
        )
        .await
        .unwrap();
        insert(
            &pool,
            "openrouter",
            "anthropic/claude-sonnet-4.6",
            "generate_content",
            400_000,
            100_000,
        )
        .await
        .unwrap();

        // One call on an unknown model — should be flagged as estimated and
        // NOT be priced as zero.
        insert(
            &pool,
            "openrouter",
            "vendor/mystery-model",
            "generate_content",
            1000,
            500,
        )
        .await
        .unwrap();

        let s = summarise(&pool).await.expect("summarise");
        assert_eq!(s.calls_30d, 3);

        // Sonnet line — exact cost match validates the input/output ratio
        // and the rounding helper at once.
        let sonnet = s
            .by_model_30d
            .iter()
            .find(|m| m.model.contains("sonnet"))
            .expect("sonnet row present");
        assert_eq!(sonnet.calls, 2);
        assert_eq!(sonnet.input_tokens, 1_000_000);
        assert_eq!(sonnet.output_tokens, 200_000);
        assert_eq!(sonnet.cost_usd, 6.00);
        assert!(!sonnet.price_estimated, "sonnet pricing is known");

        let mystery = s
            .by_model_30d
            .iter()
            .find(|m| m.model.contains("mystery"))
            .expect("mystery row present");
        assert!(mystery.price_estimated, "unknown model flagged estimated");
        assert!(mystery.cost_usd > 0.0, "fallback price must be > 0");

        // Total 30d cost = sum of both lines.
        assert!((s.cost_usd_30d - (sonnet.cost_usd + mystery.cost_usd)).abs() < 0.0001);
    }

    #[tokio::test]
    async fn summarise_excludes_calls_older_than_30_days() {
        let pool = fresh_pool().await;

        // 60 days ago: must NOT count toward the 30-day window.
        let old_ts = (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339();
        sqlx::query(
            "INSERT INTO ai_usage (occurred_at, provider, model, action, input_tokens, output_tokens) \
             VALUES (?, 'openrouter', 'anthropic/claude-sonnet-4.6', 'generate_content', 100, 50)",
        )
        .bind(&old_ts)
        .execute(&pool)
        .await
        .unwrap();

        // Today: must count.
        insert(
            &pool,
            "openrouter",
            "anthropic/claude-sonnet-4.6",
            "generate_content",
            100,
            50,
        )
        .await
        .unwrap();

        let s = summarise(&pool).await.expect("summarise");
        assert_eq!(s.calls_30d, 1, "old row must be filtered out");
    }

    #[tokio::test]
    async fn summarise_orders_models_by_cost_desc() {
        let pool = fresh_pool().await;

        // Haiku — cheap.
        insert(
            &pool,
            "openrouter",
            "anthropic/claude-haiku-latest",
            "generate_content",
            1000,
            500,
        )
        .await
        .unwrap();

        // Opus — expensive.
        insert(
            &pool,
            "openrouter",
            "anthropic/claude-opus-4.7",
            "generate_content",
            1000,
            500,
        )
        .await
        .unwrap();

        let s = summarise(&pool).await.expect("summarise");
        assert!(
            s.by_model_30d[0].cost_usd >= s.by_model_30d[1].cost_usd,
            "rows must be sorted by cost descending"
        );
        assert!(
            s.by_model_30d[0].model.contains("opus"),
            "the most expensive model lands first"
        );
    }
}
