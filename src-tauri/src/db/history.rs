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
    /// Public URL of the published post on its network. Populated by the
    /// publish flow after a successful upload (Instagram fetches via
    /// `/{media_id}?fields=permalink`, LinkedIn derives from URN at
    /// publish time). NULL for drafts and legacy published rows from
    /// before migration 017 — the frontend then falls back to a URN-
    /// derived URL or the account profile feed.
    pub published_url: Option<String>,
    /// Sibling-row group this post belongs to. Set by the multi-network
    /// composer (one parent `post_groups` row + N children with the
    /// same group_id). NULL on legacy mono-network rows and on drafts
    /// generated through the single-network path. The dashboard /
    /// calendar uses this to surface a "Groupe · N posts" badge so the
    /// user can see at a glance which drafts were generated together.
    pub group_id: Option<i64>,
    /// Count of consecutive failed publish attempts (v0.4.0 scheduler).
    /// The retry policy backs off 5 min / 30 min / 2 h and after
    /// `db::scheduler::MAX_FAILED_ATTEMPTS` flips the row to
    /// `status = 'failed'`. Default 0 on legacy rows and drafts that
    /// have never been auto-published.
    #[serde(default)]
    pub failed_attempts: i64,
    /// RFC 3339 UTC timestamp of the most recent publish attempt
    /// (success or failure). Drives the backoff window check in
    /// `db::scheduler::list_due_for_publish`. NULL on legacy rows and
    /// drafts that have never been picked up by the scheduler.
    #[serde(default)]
    pub last_attempt_at: Option<String>,
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
                scheduled_at, image_path, images, ig_media_id, account_id, published_url, group_id, failed_attempts, last_attempt_at
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
                scheduled_at, image_path, images, ig_media_id, account_id, published_url, group_id, failed_attempts, last_attempt_at
         FROM post_history ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    rows.iter().map(row_to_post_record).collect()
}

/// List posts whose effective calendar date falls within [from, to]
/// (ISO-8601 date strings).
///
/// "Effective date" is the most-concrete-event-wins precedence the
/// frontend relies on: `published_at` > `scheduled_at` > `created_at`.
/// Pre-v0.3.8 the query only filtered on `scheduled_at`/`created_at`,
/// which meant a post scheduled for May 9 and published on May 10 would
/// keep matching the May 9 range forever — confusing for users tracking
/// what actually went out.
///
/// `WHERE` and `ORDER BY` share the same `COALESCE(...)` expression —
/// SQLite's `COALESCE` returns the first non-NULL argument, which is
/// the precedence we want. `created_at` is `NOT NULL` in the schema, so
/// the expression itself is never NULL and `BETWEEN` works without a
/// guard. Keeping a single expression in both clauses avoids the
/// drift class of bugs where the filter and the sort disagree on which
/// date defines the post.
///
/// `DISTINCT` is defensive: post_history.id is unique so the JOIN-free
/// query can't currently produce duplicates, but if a future refactor
/// adds a JOIN we don't want a row to appear twice in the calendar.
pub async fn list_in_range(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> Result<Vec<PostRecord>, String> {
    let rows: Vec<SqliteRow> = sqlx::query(
        "SELECT DISTINCT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, images, ig_media_id, account_id, published_url, group_id, failed_attempts, last_attempt_at
         FROM post_history
         WHERE COALESCE(published_at, scheduled_at, created_at) BETWEEN ? AND ?
         ORDER BY COALESCE(published_at, scheduled_at, created_at) ASC",
    )
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

pub(crate) fn row_to_post_record(r: &SqliteRow) -> Result<PostRecord, String> {
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
        published_url: r.try_get("published_url").ok().flatten(),
        group_id: r.try_get("group_id").ok().flatten(),
        // New v0.4.0 columns — `unwrap_or` keeps legacy in-flight
        // queries safe if a future refactor accidentally drops them
        // from a SELECT list. SQLite NOT NULL DEFAULT 0 guarantees
        // the column itself always has a real value on disk.
        failed_attempts: r.try_get("failed_attempts").unwrap_or(0),
        last_attempt_at: r.try_get("last_attempt_at").ok().flatten(),
    })
}

/// Set the public URL of a published post (Instagram permalink or LinkedIn
/// post URL). Called by the publisher after a successful upload — separate
/// from `update_status` so the URL fetch can fail (e.g. transient Graph API
/// hiccup) without rolling back the publish.
pub async fn update_published_url(
    pool: &SqlitePool,
    id: i64,
    published_url: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE post_history SET published_url = ? WHERE id = ?")
        .bind(published_url)
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
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

    // ── list_in_range integration tests ───────────────────────────────────────
    //
    // Regression guards for the v0.3.8 calendar bucketing fix. The previous
    // SQL only filtered on scheduled_at/created_at, so a post scheduled for
    // May 9 and published on May 10 stayed glued on May 9 in the calendar
    // view forever. The fix adds published_at as the primary precedence and
    // these tests lock the contract in.

    use sqlx::sqlite::SqlitePoolOptions;

    /// Fresh in-memory pool with all migrations applied. `max_connections=1`
    /// because in-memory SQLite is private to the connection that opened it.
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

    /// Insert a post with explicit dates so we can exercise list_in_range
    /// against every combination of (published_at, scheduled_at, created_at).
    async fn insert_with_dates(
        pool: &SqlitePool,
        caption: &str,
        status: &str,
        created_at: &str,
        scheduled_at: Option<&str>,
        published_at: Option<&str>,
    ) -> i64 {
        let row = sqlx::query(
            "INSERT INTO post_history \
                (network, caption, hashtags, status, created_at, scheduled_at, published_at, images) \
             VALUES ('linkedin', ?, '[]', ?, ?, ?, ?, '[]') \
             RETURNING id",
        )
        .bind(caption)
        .bind(status)
        .bind(created_at)
        .bind(scheduled_at)
        .bind(published_at)
        .fetch_one(pool)
        .await
        .expect("insert post_history");
        row.try_get::<i64, _>("id").expect("read inserted id")
    }

    #[tokio::test]
    async fn list_in_range_buckets_published_post_on_publish_day_not_schedule_day() {
        // The v0.3.8 calendar bug: a draft scheduled May 9 and published
        // May 10 was returned for the May 9 range and stayed visible there
        // even though the post had actually shipped May 10. After the fix,
        // a calendar query for May 10 must return the post; a query for
        // May 9 must NOT.
        let pool = fresh_pool().await;
        let id = insert_with_dates(
            &pool,
            "287 tests pour un sanitizer",
            "published",
            "2026-05-09T08:00:00Z",       // created May 9
            Some("2026-05-09T09:00:00Z"), // scheduled May 9
            Some("2026-05-10T12:26:00Z"), // actually published May 10
        )
        .await;

        let may10 = list_in_range(&pool, "2026-05-10T00:00:00Z", "2026-05-10T23:59:59Z")
            .await
            .expect("list May 10");
        assert!(
            may10.iter().any(|p| p.id == id),
            "published post must be visible on its publish day (May 10)",
        );

        let may9 = list_in_range(&pool, "2026-05-09T00:00:00Z", "2026-05-09T23:59:59Z")
            .await
            .expect("list May 9");
        assert!(
            !may9.iter().any(|p| p.id == id),
            "published post must NOT be visible on its old scheduled day (May 9)",
        );
    }

    #[tokio::test]
    async fn list_in_range_falls_back_to_scheduled_then_created() {
        // Three posts covering the precedence ladder:
        //   A: published_at set    → bucketed by published_at
        //   B: scheduled, not yet published → bucketed by scheduled_at
        //   C: unscheduled draft   → bucketed by created_at
        let pool = fresh_pool().await;
        let a = insert_with_dates(
            &pool,
            "A — published",
            "published",
            "2026-05-01T00:00:00Z",
            Some("2026-05-02T00:00:00Z"),
            Some("2026-05-15T00:00:00Z"),
        )
        .await;
        let b = insert_with_dates(
            &pool,
            "B — scheduled",
            "draft",
            "2026-05-01T00:00:00Z",
            Some("2026-05-15T00:00:00Z"),
            None,
        )
        .await;
        let c = insert_with_dates(
            &pool,
            "C — unscheduled draft",
            "draft",
            "2026-05-15T00:00:00Z",
            None,
            None,
        )
        .await;

        let may15 = list_in_range(&pool, "2026-05-15T00:00:00Z", "2026-05-15T23:59:59Z")
            .await
            .expect("list May 15");
        let ids: Vec<i64> = may15.iter().map(|p| p.id).collect();
        assert!(
            ids.contains(&a),
            "A must be returned via published_at = May 15"
        );
        assert!(
            ids.contains(&b),
            "B must be returned via scheduled_at = May 15"
        );
        assert!(
            ids.contains(&c),
            "C must be returned via created_at = May 15"
        );
    }

    #[tokio::test]
    async fn list_in_range_returns_each_post_only_once() {
        // Defensive against future JOIN-introducing refactors. A post that
        // matches the range via multiple date columns (e.g. created_at AND
        // scheduled_at AND published_at all on the same day) must still
        // appear exactly once in the result.
        let pool = fresh_pool().await;
        let id = insert_with_dates(
            &pool,
            "all-three-dates same day",
            "published",
            "2026-05-10T08:00:00Z",
            Some("2026-05-10T09:00:00Z"),
            Some("2026-05-10T12:00:00Z"),
        )
        .await;

        let may10 = list_in_range(&pool, "2026-05-10T00:00:00Z", "2026-05-10T23:59:59Z")
            .await
            .expect("list May 10");
        let count = may10.iter().filter(|p| p.id == id).count();
        assert_eq!(count, 1, "post must appear exactly once, got {count}");
    }
}
