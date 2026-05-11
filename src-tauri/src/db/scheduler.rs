//! Scheduler data access for the v0.4.0 auto-publish background task.
//!
//! This module only deals with the **SQL surface** the scheduler will
//! consume. The actual background loop (poll + dispatch + retry policy)
//! lives in `src/scheduler.rs` (PR F2). Splitting the data layer here
//! keeps the SQL testable in isolation against an in-memory pool, and
//! lets PR F2 land with zero new DB plumbing.
//!
//! ## Retry policy (consumed by PR F2)
//!
//! Backoff schedule applied to `last_attempt_at`:
//!
//! | failed_attempts | next attempt window after last_attempt_at |
//! |---|---|
//! | 0 (never tried) | immediate when `scheduled_at <= now` |
//! | 1 | + 5 min |
//! | 2 | + 30 min |
//! | 3 | give up — `status` flipped to `'failed'` |
//!
//! Three is a balance: not so few that a transient outage burns
//! through, not so many that a definitively-broken account spams the
//! IG / LinkedIn API for hours. The "reconnect your account" hint
//! lands in the failure notification.

use chrono::Utc;
use sqlx::SqlitePool;

use super::history::{row_to_post_record, PostRecord};

// All items below are verified by the test suite at the bottom of the
// file but no Tauri command consumes them yet — PR F2 wires the
// background scheduler that actually calls into these. Targeted
// `#[allow(dead_code)]` keeps clippy `-D warnings` green on this
// foundation PR, same pattern as PR #56 (`db::groups`) used before
// PR #57 picked it up.

/// Hard cap on consecutive publish attempts before flipping the post
/// to `status = 'failed'`. Surface to the user via notification +
/// dashboard badge. Exposed as a const so PR F2's retry test stays
/// in sync with the SQL-level filter.
#[allow(dead_code)]
pub const MAX_FAILED_ATTEMPTS: i64 = 3;

/// Backoff windows in minutes, indexed by `failed_attempts`. The
/// scheduler reads window[failed_attempts] to compute the next eligible
/// attempt time. `failed_attempts == 0` is "never tried, fire ASAP",
/// so window[0] = 0. After MAX_FAILED_ATTEMPTS the post is given up
/// on, so window[3] is unused but defined to make indexing safe.
#[allow(dead_code)]
pub const BACKOFF_MINUTES: [i64; 4] = [0, 5, 30, 120];

/// Return every draft post whose `scheduled_at` is now or earlier AND
/// whose retry window has elapsed. Caller (the background task in PR F2)
/// then locks each row by flipping its status to `'publishing'` before
/// firing the actual publish call — that lock-row pattern prevents two
/// concurrent app launches from double-publishing the same post.
///
/// Two timestamp parameters get bound from Rust: `now_rfc3339` is the
/// reference instant for `scheduled_at` comparisons (string-equal to
/// what `chrono::Utc::now().to_rfc3339()` writes during scheduling).
/// We avoid SQLite's `datetime('now')` here because it returns the
/// format `'YYYY-MM-DD HH:MM:SS'` (space separator, no fractional, no
/// Z), which sorts lexicographically before our RFC 3339 writes
/// (`'YYYY-MM-DDTHH:MM:SS.fffZ'`, `T` > space at the same calendar
/// instant). Binding the Rust-formatted timestamp keeps producer and
/// consumer on the same string convention.
///
/// `failed_attempts` is bounded by `MAX_FAILED_ATTEMPTS` so giving-up
/// rows don't keep matching. Retry window check uses a CASE on
/// `failed_attempts` so the SQL stays one query — no per-row decision
/// in Rust.
#[allow(dead_code)]
pub async fn list_due_for_publish(pool: &SqlitePool) -> Result<Vec<PostRecord>, String> {
    let now_rfc3339 = Utc::now().to_rfc3339();
    let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, images, ig_media_id, account_id, published_url,
                group_id, failed_attempts, last_attempt_at
         FROM post_history
         WHERE status = 'draft'
           AND scheduled_at IS NOT NULL
           AND scheduled_at <= ?
           AND failed_attempts < ?
           AND (
               last_attempt_at IS NULL
               OR datetime(last_attempt_at, '+' || (
                   CASE failed_attempts
                       WHEN 0 THEN 0
                       WHEN 1 THEN 5
                       WHEN 2 THEN 30
                       ELSE 120
                   END
               ) || ' minutes') <= datetime(?)
           )
         ORDER BY scheduled_at ASC",
    )
    .bind(&now_rfc3339)
    .bind(MAX_FAILED_ATTEMPTS)
    .bind(&now_rfc3339)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    rows.iter().map(row_to_post_record).collect()
}

/// Flip a draft to `status = 'publishing'` atomically. Used by the
/// scheduler to "lock" a row before firing the publish call so a second
/// app launch racing on the same row sees the lock and skips. Returns
/// `true` when the row was successfully locked (i.e. the UPDATE
/// affected exactly one row that was previously a draft), `false` when
/// another worker already grabbed it.
///
/// The atomicity comes from the `WHERE status = 'draft'` clause — even
/// without explicit row-level locking, SQLite's serialized writes
/// ensure two simultaneous attempts can't both flip the same row from
/// draft to publishing.
#[allow(dead_code)]
pub async fn try_lock_for_publish(pool: &SqlitePool, post_id: i64) -> Result<bool, String> {
    let result = sqlx::query(
        "UPDATE post_history
         SET status = 'publishing', last_attempt_at = ?
         WHERE id = ? AND status = 'draft'",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(post_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(result.rows_affected() == 1)
}

/// Record a publish-attempt failure: increment `failed_attempts`, set
/// `last_attempt_at` to the present, and either return the row to
/// `'draft'` (so the next polling pass picks it up after the backoff)
/// or flip to `'failed'` when we've exhausted the budget.
///
/// Should be called by the scheduler in PR F2 when the actual publish
/// call (`publish_post` / `publish_linkedin_post`) returned `Err`.
#[allow(dead_code)]
pub async fn mark_publish_attempt_failed(
    pool: &SqlitePool,
    post_id: i64,
) -> Result<PublishFailureOutcome, String> {
    let now = Utc::now().to_rfc3339();
    // Single UPDATE that does the accounting: the CASE flips the
    // status to 'failed' once the increment crosses MAX_FAILED_ATTEMPTS,
    // otherwise returns the row to draft so the next polling pass can
    // pick it up after the backoff window.
    sqlx::query(
        "UPDATE post_history
         SET failed_attempts = failed_attempts + 1,
             last_attempt_at = ?,
             status = CASE
                 WHEN failed_attempts + 1 >= ? THEN 'failed'
                 ELSE 'draft'
             END
         WHERE id = ?",
    )
    .bind(&now)
    .bind(MAX_FAILED_ATTEMPTS)
    .bind(post_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Look up the new state to tell the scheduler whether to schedule a
    // retry or surface a final-failure notification.
    let (failed_attempts, status): (i64, String) =
        sqlx::query_as("SELECT failed_attempts, status FROM post_history WHERE id = ?")
            .bind(post_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    if status == "failed" {
        Ok(PublishFailureOutcome::GaveUp { failed_attempts })
    } else {
        Ok(PublishFailureOutcome::WillRetry { failed_attempts })
    }
}

/// Reset retry bookkeeping when the user manually reschedules a row
/// that previously failed N times — they've presumably fixed the
/// underlying problem (reconnected account, etc.) and want a fresh
/// budget. Called by the Calendar reschedule flow in PR F3.
#[allow(dead_code)]
pub async fn reset_retry_state(pool: &SqlitePool, post_id: i64) -> Result<(), String> {
    sqlx::query(
        "UPDATE post_history
         SET failed_attempts = 0,
             last_attempt_at = NULL,
             status = 'draft'
         WHERE id = ?",
    )
    .bind(post_id)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Outcome reported by [`mark_publish_attempt_failed`]. Tells the
/// scheduler whether the post is still in the retry budget or has
/// been flipped to a permanent `'failed'` state.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishFailureOutcome {
    /// Post returned to `status = 'draft'` and will be retried after
    /// the backoff window. Includes the current `failed_attempts` so
    /// the scheduler can log the attempt sequence.
    WillRetry { failed_attempts: i64 },
    /// Post has exhausted the retry budget and is now in
    /// `status = 'failed'`. The user must intervene (reconnect
    /// account / reschedule manually). The scheduler should surface
    /// this via a desktop notification.
    GaveUp { failed_attempts: i64 },
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
            .expect("migrations apply cleanly");
        pool
    }

    /// Insert a scheduled draft with explicit retry state for the test.
    /// `scheduled_at_offset_min` is added to "now" — negative values
    /// produce a post that's overdue.
    async fn insert_scheduled(
        pool: &SqlitePool,
        scheduled_at_offset_min: i64,
        failed_attempts: i64,
        last_attempt_offset_min: Option<i64>,
    ) -> i64 {
        let now = Utc::now();
        let scheduled_at = now + chrono::Duration::minutes(scheduled_at_offset_min);
        let last_attempt =
            last_attempt_offset_min.map(|m| (now + chrono::Duration::minutes(m)).to_rfc3339());

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO post_history
                (network, caption, hashtags, status, created_at, scheduled_at,
                 images, failed_attempts, last_attempt_at)
             VALUES ('instagram', 'Test scheduled post', '[]', 'draft',
                     ?, ?, '[]', ?, ?)
             RETURNING id",
        )
        .bind(now.to_rfc3339())
        .bind(scheduled_at.to_rfc3339())
        .bind(failed_attempts)
        .bind(last_attempt)
        .fetch_one(pool)
        .await
        .expect("insert scheduled");
        id
    }

    #[tokio::test]
    async fn list_due_returns_overdue_drafts_only() {
        // Two posts: one overdue by 5 min (should match), one due in
        // 10 min (should NOT match — the scheduler only fires past-due).
        let pool = fresh_pool().await;
        let overdue_id = insert_scheduled(&pool, -5, 0, None).await;
        let _future_id = insert_scheduled(&pool, 10, 0, None).await;

        let due = list_due_for_publish(&pool).await.expect("list due");
        assert_eq!(due.len(), 1, "only the overdue post must match");
        assert_eq!(due[0].id, overdue_id);
    }

    #[tokio::test]
    async fn list_due_skips_posts_in_backoff_window() {
        // Post that's overdue AND failed once 2 min ago: must NOT
        // match — the 5-min backoff for attempts=1 hasn't elapsed.
        let pool = fresh_pool().await;
        let _within_backoff = insert_scheduled(&pool, -10, 1, Some(-2)).await;
        let due = list_due_for_publish(&pool).await.expect("list");
        assert!(
            due.is_empty(),
            "post still in 5-min backoff after 1 failure must not match"
        );
    }

    #[tokio::test]
    async fn list_due_includes_posts_past_backoff_window() {
        // Post overdue, failed once 10 min ago. The 5-min backoff is
        // long over → must match. Locks the contract: the scheduler
        // doesn't need to know the backoff schedule by heart, it just
        // calls list_due and the SQL does the math.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -30, 1, Some(-10)).await;
        let due = list_due_for_publish(&pool).await.expect("list");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, id);
    }

    #[tokio::test]
    async fn list_due_excludes_posts_at_or_beyond_max_attempts() {
        // failed_attempts == MAX_FAILED_ATTEMPTS means the row was
        // flipped to 'failed' by mark_publish_attempt_failed — should
        // never be picked up again until the user manually resets.
        let pool = fresh_pool().await;
        let _given_up = insert_scheduled(&pool, -60, MAX_FAILED_ATTEMPTS, Some(-60)).await;
        let due = list_due_for_publish(&pool).await.expect("list");
        assert!(due.is_empty(), "given-up post must not be picked up");
    }

    #[tokio::test]
    async fn list_due_skips_non_draft_statuses() {
        // 'publishing' (locked by another worker) and 'published'
        // (already out) must never match. The status filter is the
        // first line of defense against concurrent app launches
        // racing on the same row.
        let pool = fresh_pool().await;
        let now = Utc::now().to_rfc3339();
        let scheduled = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        for status in ["publishing", "published", "failed"] {
            sqlx::query(
                "INSERT INTO post_history
                    (network, caption, hashtags, status, created_at, scheduled_at, images)
                 VALUES ('instagram', 'x', '[]', ?, ?, ?, '[]')",
            )
            .bind(status)
            .bind(&now)
            .bind(&scheduled)
            .execute(&pool)
            .await
            .expect("insert");
        }
        let due = list_due_for_publish(&pool).await.expect("list");
        assert!(due.is_empty(), "only 'draft' status must match");
    }

    #[tokio::test]
    async fn try_lock_flips_draft_to_publishing_and_returns_true() {
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5, 0, None).await;
        let locked = try_lock_for_publish(&pool, id).await.expect("lock");
        assert!(locked);

        let status: String = sqlx::query_scalar("SELECT status FROM post_history WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("get");
        assert_eq!(status, "publishing");
    }

    #[tokio::test]
    async fn try_lock_returns_false_when_already_locked() {
        // Race: another worker already flipped the row to 'publishing'.
        // Second try_lock must return false so the loser skips.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5, 0, None).await;
        let first = try_lock_for_publish(&pool, id).await.expect("first lock");
        let second = try_lock_for_publish(&pool, id).await.expect("second");
        assert!(first);
        assert!(
            !second,
            "second concurrent lock must return false, not double-publish"
        );
    }

    #[tokio::test]
    async fn mark_failed_returns_will_retry_until_threshold() {
        // First two failures → WillRetry, third → GaveUp. The status
        // flip to 'failed' must happen exactly at the threshold and
        // not one attempt early or late.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5, 0, None).await;

        let r1 = mark_publish_attempt_failed(&pool, id).await.expect("1");
        assert_eq!(r1, PublishFailureOutcome::WillRetry { failed_attempts: 1 });

        let r2 = mark_publish_attempt_failed(&pool, id).await.expect("2");
        assert_eq!(r2, PublishFailureOutcome::WillRetry { failed_attempts: 2 });

        let r3 = mark_publish_attempt_failed(&pool, id).await.expect("3");
        assert_eq!(r3, PublishFailureOutcome::GaveUp { failed_attempts: 3 });

        let status: String = sqlx::query_scalar("SELECT status FROM post_history WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("get");
        assert_eq!(status, "failed");
    }

    #[tokio::test]
    async fn reset_retry_state_returns_post_to_draft_with_zero_attempts() {
        // User-side recovery flow: after fixing their reconnection,
        // they reschedule a 'failed' post. Reset must clear the
        // counter and the timestamp so the next polling pass treats
        // it as fresh.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -60, 3, Some(-30)).await;
        // Manually set status='failed' to mirror what the scheduler
        // would have done after exhausting the budget.
        sqlx::query("UPDATE post_history SET status = 'failed' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .expect("flip to failed");

        reset_retry_state(&pool, id).await.expect("reset");

        let row: (String, i64, Option<String>) = sqlx::query_as(
            "SELECT status, failed_attempts, last_attempt_at \
             FROM post_history WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .expect("read");
        assert_eq!(row.0, "draft");
        assert_eq!(row.1, 0);
        assert!(row.2.is_none());
    }

    #[tokio::test]
    async fn backoff_minutes_table_matches_documented_policy() {
        // Lock the documented policy in code. Changing the constants
        // is fine, but it must be a conscious change visible in this
        // test's diff — not a silent drift between docs and implementation.
        assert_eq!(BACKOFF_MINUTES, [0, 5, 30, 120]);
        assert_eq!(MAX_FAILED_ATTEMPTS, 3);
        assert_eq!(BACKOFF_MINUTES.len() as i64, MAX_FAILED_ATTEMPTS + 1);
    }
}
