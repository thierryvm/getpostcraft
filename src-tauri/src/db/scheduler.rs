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
/// `now_rfc3339` is bound from Rust as the reference instant for
/// `scheduled_at` comparisons (string-equal to what
/// `chrono::Utc::now().to_rfc3339()` writes during scheduling). We avoid
/// SQLite's `datetime('now')` here because it returns the format
/// `'YYYY-MM-DD HH:MM:SS'` (space separator, no fractional, no Z), which
/// sorts lexicographically before our RFC 3339 writes
/// (`'YYYY-MM-DDTHH:MM:SS.fffZ'`, `T` > space at the same calendar
/// instant). Binding the Rust-formatted timestamp keeps producer and
/// consumer on the same string convention.
///
/// `failed_attempts` is bounded by `MAX_FAILED_ATTEMPTS` so giving-up
/// rows don't keep matching. The backoff window check is applied in
/// Rust against [`BACKOFF_MINUTES`] (single source of truth), so the
/// SQL no longer duplicates the retry policy in a `CASE`. For the
/// expected scheduler load (a handful of overdue posts at most), the
/// extra rows fetched are negligible vs the maintainability win.
#[allow(dead_code)]
pub async fn list_due_for_publish(pool: &SqlitePool) -> Result<Vec<PostRecord>, String> {
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, images, ig_media_id, account_id, published_url,
                group_id, failed_attempts, last_attempt_at
         FROM post_history
         WHERE status = 'draft'
           AND scheduled_at IS NOT NULL
           AND scheduled_at <= ?
           AND failed_attempts < ?
         ORDER BY scheduled_at ASC",
    )
    .bind(&now_rfc3339)
    .bind(MAX_FAILED_ATTEMPTS)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let due = rows
        .iter()
        .map(row_to_post_record)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|post| backoff_window_elapsed(post, now))
        .collect();
    Ok(due)
}

/// Returns true when `post` has either never been attempted or its
/// `last_attempt_at + BACKOFF_MINUTES[failed_attempts]` is now in the
/// past. Indexing is saturated so an unexpected `failed_attempts`
/// above the table length falls back to the longest backoff (defensive,
/// list_due already filters `failed_attempts < MAX_FAILED_ATTEMPTS`).
fn backoff_window_elapsed(post: &PostRecord, now: chrono::DateTime<Utc>) -> bool {
    let Some(last_attempt) = post.last_attempt_at.as_deref() else {
        return true;
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(last_attempt) else {
        // Malformed timestamp — fail open so the post can be retried;
        // the actual publish call will surface any real problem.
        return true;
    };
    let attempts_idx = post.failed_attempts.max(0) as usize;
    let window_min = BACKOFF_MINUTES
        .get(attempts_idx)
        .copied()
        .unwrap_or_else(|| *BACKOFF_MINUTES.last().unwrap_or(&0));
    parsed.with_timezone(&Utc) + chrono::Duration::minutes(window_min) <= now
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
/// call (`publish_post` / `publish_linkedin_post`) returned `Err`. The
/// `WHERE status = 'publishing'` guard mirrors [`try_lock_for_publish`]:
/// only an in-flight publish can fail. If the caller is racing — e.g.
/// a user manually rescheduled the row between lock and failure — the
/// UPDATE affects zero rows and we return [`PublishFailureOutcome::Skipped`]
/// so the scheduler can log the no-op without corrupting retry state.
///
/// Uses `UPDATE ... RETURNING` to fold the state read into the same
/// statement (1 round-trip instead of 2) and to keep the "before vs
/// after" of `failed_attempts` from being split across two queries.
#[allow(dead_code)]
pub async fn mark_publish_attempt_failed(
    pool: &SqlitePool,
    post_id: i64,
) -> Result<PublishFailureOutcome, String> {
    let now = Utc::now().to_rfc3339();
    let row: Option<(i64, String)> = sqlx::query_as(
        "UPDATE post_history
         SET failed_attempts = failed_attempts + 1,
             last_attempt_at = ?,
             status = CASE
                 WHEN failed_attempts + 1 >= ? THEN 'failed'
                 ELSE 'draft'
             END
         WHERE id = ? AND status = 'publishing'
         RETURNING failed_attempts, status",
    )
    .bind(&now)
    .bind(MAX_FAILED_ATTEMPTS)
    .bind(post_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    match row {
        None => Ok(PublishFailureOutcome::Skipped),
        Some((failed_attempts, status)) if status == "failed" => {
            Ok(PublishFailureOutcome::GaveUp { failed_attempts })
        }
        Some((failed_attempts, _)) => Ok(PublishFailureOutcome::WillRetry { failed_attempts }),
    }
}

/// Reset retry bookkeeping when the user manually reschedules a row
/// that previously failed N times — they've presumably fixed the
/// underlying problem (reconnected account, etc.) and want a fresh
/// budget. Called by the Calendar reschedule flow in PR F3.
///
/// The `WHERE status IN ('draft', 'failed')` guard is critical: without
/// it a misfired reset on a `'published'` row would silently un-publish
/// it from the user's perspective, and a reset on a `'publishing'` row
/// would race the scheduler. Returns `true` when exactly one row was
/// reset, `false` when the guard rejected the call (already published,
/// currently publishing, or unknown id) so the caller can surface the
/// no-op to the user.
#[allow(dead_code)]
pub async fn reset_retry_state(pool: &SqlitePool, post_id: i64) -> Result<bool, String> {
    let result = sqlx::query(
        "UPDATE post_history
         SET failed_attempts = 0,
             last_attempt_at = NULL,
             status = 'draft'
         WHERE id = ? AND status IN ('draft', 'failed')",
    )
    .bind(post_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(result.rows_affected() == 1)
}

/// Outcome reported by [`mark_publish_attempt_failed`]. Tells the
/// scheduler whether the post is still in the retry budget, has been
/// flipped to a permanent `'failed'` state, or no action was taken
/// because the row was not in `'publishing'` status.
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
    /// No-op: the row was not in `'publishing'` status, so no retry
    /// state was touched. The scheduler should log this (typically a
    /// race with a manual reschedule) and move on without notifying.
    Skipped,
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

    /// Move a row from 'draft' through 'publishing' the way the real
    /// scheduler does, so `mark_publish_attempt_failed` (which now
    /// requires `status = 'publishing'`) sees the correct precondition.
    async fn lock_for_test(pool: &SqlitePool, id: i64) {
        let locked = try_lock_for_publish(pool, id).await.expect("lock");
        assert!(locked, "test setup expected fresh draft to lock cleanly");
    }

    #[tokio::test]
    async fn mark_failed_returns_will_retry_until_threshold() {
        // First two failures → WillRetry, third → GaveUp. The status
        // flip to 'failed' must happen exactly at the threshold and
        // not one attempt early or late. Each iteration re-locks
        // because the row goes back to 'draft' between attempts.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5, 0, None).await;

        lock_for_test(&pool, id).await;
        let r1 = mark_publish_attempt_failed(&pool, id).await.expect("1");
        assert_eq!(r1, PublishFailureOutcome::WillRetry { failed_attempts: 1 });

        lock_for_test(&pool, id).await;
        let r2 = mark_publish_attempt_failed(&pool, id).await.expect("2");
        assert_eq!(r2, PublishFailureOutcome::WillRetry { failed_attempts: 2 });

        lock_for_test(&pool, id).await;
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
    async fn mark_failed_skips_when_status_is_not_publishing() {
        // Regression: caller mis-fires mark_publish_attempt_failed on
        // a draft row (e.g. another worker already unlocked, or the
        // user rescheduled between lock and dispatch). The guard must
        // prevent corrupting retry state — failed_attempts stays put.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5, 0, None).await;

        let outcome = mark_publish_attempt_failed(&pool, id).await.expect("mark");
        assert_eq!(outcome, PublishFailureOutcome::Skipped);

        let (failed_attempts, status): (i64, String) =
            sqlx::query_as("SELECT failed_attempts, status FROM post_history WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("get");
        assert_eq!(
            failed_attempts, 0,
            "guard must protect failed_attempts from misfire"
        );
        assert_eq!(status, "draft");
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

        let reset_ok = reset_retry_state(&pool, id).await.expect("reset");
        assert!(reset_ok, "reset on a failed row must succeed");

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
    async fn reset_retry_state_rejects_published_row() {
        // Critical regression: an accidental reset call on a published
        // post must NOT silently un-publish it from the user's POV.
        // The guard catches it and reports the no-op via `false`.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -60, 0, None).await;
        sqlx::query("UPDATE post_history SET status = 'published', published_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&pool)
            .await
            .expect("flip to published");

        let reset_ok = reset_retry_state(&pool, id).await.expect("reset");
        assert!(
            !reset_ok,
            "reset on a published row must report no-op, not lie about success"
        );

        let status: String = sqlx::query_scalar("SELECT status FROM post_history WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("get");
        assert_eq!(status, "published", "published row must stay published");
    }

    #[tokio::test]
    async fn reset_retry_state_rejects_publishing_row() {
        // Race protection: a reset issued while the scheduler holds
        // the row in 'publishing' must not blow away its lock.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5, 0, None).await;
        lock_for_test(&pool, id).await;

        let reset_ok = reset_retry_state(&pool, id).await.expect("reset");
        assert!(!reset_ok, "reset must not race the scheduler's lock");

        let status: String = sqlx::query_scalar("SELECT status FROM post_history WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("get");
        assert_eq!(status, "publishing");
    }

    #[tokio::test]
    async fn backoff_minutes_table_matches_documented_policy() {
        // Lock the documented policy in code. Changing the constants
        // is fine, but it must be a conscious change visible in this
        // test's diff — not a silent drift between docs and implementation.
        assert_eq!(BACKOFF_MINUTES, [0, 5, 30, 120]);
        assert_eq!(MAX_FAILED_ATTEMPTS, 3);
        // Backoff table must cover index 0..=MAX_FAILED_ATTEMPTS inclusive
        // so backoff_window_elapsed never reads out of bounds.
        assert!(BACKOFF_MINUTES.len() as i64 > MAX_FAILED_ATTEMPTS);
    }
}
