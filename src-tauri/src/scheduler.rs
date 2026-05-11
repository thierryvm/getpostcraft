//! Background scheduler that polls SQLite for due drafts and dispatches
//! them through the existing publish commands.
//!
//! ## Architecture
//!
//! The scheduler is split in three layers, deliberately small:
//!
//! 1. `tick(pool, dispatcher, notifier)` — one polling iteration. Reads
//!    `db::scheduler::list_due_for_publish` and routes each due post
//!    through `process_post`. Pure logic, no `AppHandle` — testable
//!    with an in-memory SQLite pool and mock dispatcher/notifier.
//!
//! 2. `process_post(...)` — lock the row, hand off to the dispatcher,
//!    record failures via `mark_publish_attempt_failed`, and fire the
//!    "gave up" notification once the retry budget is exhausted. The
//!    happy path delegates the DB status flip (→ 'published') to the
//!    underlying `commands::publisher::publish_post`, which already does
//!    that work — duplicating it here would risk drift.
//!
//! 3. `spawn(handle)` — the only entry point with a real `AppHandle`.
//!    Builds the production `TauriDispatcher` + `TauriNotifier` and
//!    detaches a tokio task that ticks every [`TICK_INTERVAL_SECS`]
//!    seconds. Lives for the rest of the process lifetime.
//!
//! ## Why traits and not a closure
//!
//! `PublishDispatcher` / `Notifier` traits exist purely so the tick
//! loop can be exercised without an `AppHandle`. The Tauri runtime
//! can't be stood up cheaply in a `#[tokio::test]`, so the traits give
//! us a seam for mocks. The production implementations are thin shells
//! that delegate to `commands::publisher::*` and the notification plugin.
//!
//! ## Why no in-Rust retry timer
//!
//! All retry timing lives in `db::scheduler::BACKOFF_MINUTES`, consulted
//! by `list_due_for_publish` at every tick. The scheduler itself is a
//! dumb pump: it doesn't track per-post timers, doesn't keep state
//! across ticks, and survives app restarts without losing track of
//! anything (the source of truth is on disk).

use std::sync::Arc;

use async_trait::async_trait;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::time::{sleep, Duration};

use crate::db::history::PostRecord;
use crate::db::scheduler as db_scheduler;
use crate::db::scheduler::{PublishFailureOutcome, MAX_FAILED_ATTEMPTS};
use crate::state::AppState;

/// Poll interval between scheduler ticks. 60 seconds matches the UI's
/// minute-grained scheduling granularity — anything finer would burn
/// the SQLite WAL for no user-visible benefit.
const TICK_INTERVAL_SECS: u64 = 60;

/// Initial delay after process start before the first tick. Lets the
/// first window paint and the auto-backup task settle before the
/// scheduler adds its own SQLite reads on top.
const INITIAL_DELAY_SECS: u64 = 30;

/// Pre-publish guard: refuse a dispatch when the OAuth access token is
/// expired or about to expire within this window. Saves a round-trip to
/// Meta/LinkedIn that would 401 anyway, and surfaces a clearer error
/// in the retry log ("token expired") instead of a generic HTTP failure.
const TOKEN_EXPIRY_GRACE_MIN: i64 = 5;

#[async_trait]
pub trait PublishDispatcher: Send + Sync {
    /// Publish `post` on whatever network it targets. The dispatcher is
    /// expected to update the row's status to `'published'` itself —
    /// the scheduler does not touch the success path of the DB so the
    /// existing publish commands remain the single source of truth.
    async fn publish(&self, post: &PostRecord) -> Result<(), String>;
}

pub trait Notifier: Send + Sync {
    /// Fired once when a post has exhausted [`MAX_FAILED_ATTEMPTS`] and
    /// been flipped to `'failed'` — the user needs to intervene
    /// (reconnect account, reschedule manually).
    fn notify_gave_up(&self, post: &PostRecord, failed_attempts: i64);
}

/// Production dispatcher: routes by `post.network` to the existing
/// publish commands. Pulls `AppState` from the handle on each call so
/// the scheduler doesn't hold a long-lived `State<'_, AppState>` across
/// awaits (the Tauri state guard is a short-lived borrow).
pub struct TauriDispatcher {
    handle: AppHandle,
}

#[async_trait]
impl PublishDispatcher for TauriDispatcher {
    async fn publish(&self, post: &PostRecord) -> Result<(), String> {
        // Pre-check token expiry. If the OAuth access token already
        // expired (or expires within the grace window), short-circuit
        // here with a clear error instead of letting Meta/LinkedIn
        // return a generic 401. The dispatch still counts as a failed
        // attempt, so the retry budget burns down even on the dead-token
        // path — the user gets the GaveUp notification eventually
        // instead of silently retrying a doomed token forever.
        let state: tauri::State<AppState> = self.handle.state();
        pre_check_token_expiry(&state.db, post).await?;

        match post.network.as_str() {
            "instagram" => crate::commands::publisher::publish_post(post.id, state)
                .await
                .map(|_| ()),
            "linkedin" => crate::commands::publisher::publish_linkedin_post(post.id, state)
                .await
                .map(|_| ()),
            other => Err(format!("unknown network for scheduled publish: {other}")),
        }
    }
}

/// Look up the account the post is attached to and refuse the dispatch
/// when its `token_expires_at` is in the past (or inside the grace
/// window). Falls open — `Ok(())` — when the account has no expiry on
/// file (legacy rows from before migration 014) or the post has no
/// `account_id`; the publish call itself will surface any real auth
/// problem in those cases.
async fn pre_check_token_expiry(pool: &sqlx::SqlitePool, post: &PostRecord) -> Result<(), String> {
    let Some(account_id) = post.account_id else {
        return Ok(());
    };
    let account = match crate::db::accounts::get_by_id(pool, account_id).await {
        Ok(a) => a,
        Err(_) => return Ok(()), // let publish surface the real error
    };
    let Some(expires_at) = account.token_expires_at.as_deref() else {
        return Ok(());
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(expires_at) else {
        return Ok(());
    };
    let cutoff = chrono::Utc::now() + chrono::Duration::minutes(TOKEN_EXPIRY_GRACE_MIN);
    if parsed.with_timezone(&chrono::Utc) <= cutoff {
        return Err(format!(
            "OAuth token for account {account_id} expired at {expires_at} — reconnect required"
        ));
    }
    Ok(())
}

/// Production notifier: shows a desktop notification via
/// `tauri-plugin-notification`. Failures from the platform notification
/// backend are logged (not propagated) — a missing notification must
/// never abort the publish bookkeeping.
pub struct TauriNotifier {
    handle: AppHandle,
}

impl Notifier for TauriNotifier {
    fn notify_gave_up(&self, post: &PostRecord, failed_attempts: i64) {
        let network_label = match post.network.as_str() {
            "instagram" => "Instagram",
            "linkedin" => "LinkedIn",
            other => other,
        };
        let body = format!(
            "Le post #{} n'a pas pu être publié sur {network_label} après {failed_attempts} \
             tentatives. Reconnecte ton compte et reprogramme-le depuis le calendrier.",
            post.id
        );
        if let Err(e) = self
            .handle
            .notification()
            .builder()
            .title("Publication automatique : échec")
            .body(body)
            .show()
        {
            log::warn!("scheduler: notification show failed: {e}");
        }
    }
}

/// Spawn the scheduler as a detached background task. Returns
/// immediately; the task lives for the rest of the process lifetime.
/// Safe to call exactly once from `lib.rs::setup`.
pub fn spawn(handle: AppHandle) {
    let dispatcher: Arc<dyn PublishDispatcher> = Arc::new(TauriDispatcher {
        handle: handle.clone(),
    });
    let notifier: Arc<dyn Notifier> = Arc::new(TauriNotifier {
        handle: handle.clone(),
    });

    tauri::async_runtime::spawn(async move {
        // Small initial delay so the scheduler doesn't fight with the
        // first window paint / auto-backup / pricing refresh tasks.
        sleep(Duration::from_secs(INITIAL_DELAY_SECS)).await;
        loop {
            let state: tauri::State<AppState> = handle.state();
            tick(&state.db, dispatcher.as_ref(), notifier.as_ref()).await;
            sleep(Duration::from_secs(TICK_INTERVAL_SECS)).await;
        }
    });
}

/// Run one scheduler iteration: list due posts, then for each one
/// lock-and-dispatch through the supplied dispatcher. Public for
/// integration tests that drive the loop one tick at a time without
/// waiting on the real interval.
pub async fn tick(
    pool: &sqlx::SqlitePool,
    dispatcher: &dyn PublishDispatcher,
    notifier: &dyn Notifier,
) {
    let due = match db_scheduler::list_due_for_publish(pool).await {
        Ok(d) => d,
        Err(e) => {
            log::warn!("scheduler: list_due failed: {e}");
            return;
        }
    };
    if due.is_empty() {
        return;
    }
    log::info!("scheduler: tick — {} post(s) due", due.len());

    for post in due {
        process_post(pool, &post, dispatcher, notifier).await;
    }
}

async fn process_post(
    pool: &sqlx::SqlitePool,
    post: &PostRecord,
    dispatcher: &dyn PublishDispatcher,
    notifier: &dyn Notifier,
) {
    // Atomic lock: flip 'draft' → 'publishing' so a second instance
    // can't double-publish. `try_lock_for_publish` returns false when
    // another worker already grabbed it.
    match db_scheduler::try_lock_for_publish(pool, post.id).await {
        Ok(true) => {}
        Ok(false) => {
            log::debug!(
                "scheduler: post {} already locked by another worker — skipping",
                post.id
            );
            return;
        }
        Err(e) => {
            log::error!("scheduler: try_lock failed on post {}: {e}", post.id);
            return;
        }
    }

    // Dispatch to the network-specific publish command. The publish
    // command flips status to 'published' itself on success, so the
    // scheduler does nothing on the happy path beyond logging.
    match dispatcher.publish(post).await {
        Ok(()) => {
            log::info!("scheduler: post {} published on {}", post.id, post.network);
        }
        Err(e) => {
            log::warn!("scheduler: publish failed for post {}: {e}", post.id);
            match db_scheduler::mark_publish_attempt_failed(pool, post.id).await {
                Ok(PublishFailureOutcome::GaveUp { failed_attempts }) => {
                    log::warn!(
                        "scheduler: post {} gave up after {failed_attempts} attempts",
                        post.id
                    );
                    notifier.notify_gave_up(post, failed_attempts);
                }
                Ok(PublishFailureOutcome::WillRetry { failed_attempts }) => {
                    log::info!(
                        "scheduler: post {} will retry (attempt {failed_attempts}/{MAX_FAILED_ATTEMPTS})",
                        post.id
                    );
                }
                Ok(PublishFailureOutcome::Skipped) => {
                    log::debug!(
                        "scheduler: mark_failed skipped on post {} (status race)",
                        post.id
                    );
                }
                Err(me) => {
                    log::error!("scheduler: mark_failed failed on post {}: {me}", post.id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Mock dispatcher that records every call and either succeeds or
    /// fails as configured. On success it mimics the real
    /// `publish_post` by flipping the row to `'published'` in SQLite,
    /// so the next `list_due_for_publish` doesn't pick it back up.
    struct MockDispatcher {
        calls: AtomicUsize,
        outcome: MockOutcome,
        pool: SqlitePool,
    }

    enum MockOutcome {
        Success,
        Failure,
    }

    #[async_trait]
    impl PublishDispatcher for MockDispatcher {
        async fn publish(&self, post: &PostRecord) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.outcome {
                MockOutcome::Success => {
                    // Mirror the real publish command's status flip so
                    // a subsequent list_due ignores this row.
                    let now = Utc::now().to_rfc3339();
                    sqlx::query(
                        "UPDATE post_history SET status = 'published', published_at = ? \
                         WHERE id = ?",
                    )
                    .bind(&now)
                    .bind(post.id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                    Ok(())
                }
                MockOutcome::Failure => Err("mock publish failure".to_string()),
            }
        }
    }

    /// Notifier mock that just counts gave-up calls so we can assert
    /// "notification fired exactly once after the third failure".
    struct MockNotifier {
        gave_up_calls: AtomicUsize,
        last_post_id: Mutex<Option<i64>>,
    }

    impl Notifier for MockNotifier {
        fn notify_gave_up(&self, post: &PostRecord, _failed_attempts: i64) {
            self.gave_up_calls.fetch_add(1, Ordering::SeqCst);
            *self.last_post_id.lock().unwrap() = Some(post.id);
        }
    }

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

    /// Insert a scheduled draft with the given overdue offset (negative
    /// = past). Matches the shape used by the data-layer tests so the
    /// fixtures stay aligned across the two scheduler files.
    async fn insert_scheduled(pool: &SqlitePool, overdue_min: i64) -> i64 {
        let now = Utc::now();
        let scheduled = now + chrono::Duration::minutes(overdue_min);
        sqlx::query_scalar(
            "INSERT INTO post_history
                (network, caption, hashtags, status, created_at, scheduled_at, images,
                 failed_attempts, last_attempt_at)
             VALUES ('instagram', 'scheduled test', '[]', 'draft', ?, ?, '[]', 0, NULL)
             RETURNING id",
        )
        .bind(now.to_rfc3339())
        .bind(scheduled.to_rfc3339())
        .fetch_one(pool)
        .await
        .expect("insert scheduled")
    }

    async fn fetch_status(pool: &SqlitePool, id: i64) -> (String, i64) {
        sqlx::query_as("SELECT status, failed_attempts FROM post_history WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("fetch status")
    }

    fn mock_pair(pool: &SqlitePool, outcome: MockOutcome) -> (MockDispatcher, MockNotifier) {
        let dispatcher = MockDispatcher {
            calls: AtomicUsize::new(0),
            outcome,
            pool: pool.clone(),
        };
        let notifier = MockNotifier {
            gave_up_calls: AtomicUsize::new(0),
            last_post_id: Mutex::new(None),
        };
        (dispatcher, notifier)
    }

    #[tokio::test]
    async fn tick_with_no_due_posts_is_noop() {
        let pool = fresh_pool().await;
        let (disp, notif) = mock_pair(&pool, MockOutcome::Success);
        tick(&pool, &disp, &notif).await;
        assert_eq!(disp.calls.load(Ordering::SeqCst), 0);
        assert_eq!(notif.gave_up_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn tick_publishes_overdue_draft_and_flips_status() {
        // The headline F2 E2E: insert an overdue draft, tick once,
        // assert the row is now 'published'. Mock dispatcher does the
        // status flip on success, mirroring the real publish flow.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5).await;
        let (disp, notif) = mock_pair(&pool, MockOutcome::Success);

        tick(&pool, &disp, &notif).await;

        assert_eq!(disp.calls.load(Ordering::SeqCst), 1);
        let (status, attempts) = fetch_status(&pool, id).await;
        assert_eq!(status, "published");
        assert_eq!(
            attempts, 0,
            "successful publish must not increment retry count"
        );
        assert_eq!(notif.gave_up_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn tick_records_failure_and_returns_to_draft_until_threshold() {
        // First failure: row goes back to 'draft' with failed_attempts=1,
        // no notification fired (still in the retry budget).
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5).await;
        let (disp, notif) = mock_pair(&pool, MockOutcome::Failure);

        tick(&pool, &disp, &notif).await;

        assert_eq!(disp.calls.load(Ordering::SeqCst), 1);
        let (status, attempts) = fetch_status(&pool, id).await;
        assert_eq!(status, "draft");
        assert_eq!(attempts, 1);
        assert_eq!(
            notif.gave_up_calls.load(Ordering::SeqCst),
            0,
            "first failure must not notify — still within retry budget"
        );
    }

    #[tokio::test]
    async fn tick_skips_already_locked_post() {
        // Pre-flip the row to 'publishing' to simulate another instance
        // holding the lock. tick must skip it without calling publish.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -5).await;
        sqlx::query("UPDATE post_history SET status = 'publishing' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .expect("flip to publishing");

        let (disp, notif) = mock_pair(&pool, MockOutcome::Success);
        tick(&pool, &disp, &notif).await;
        assert_eq!(
            disp.calls.load(Ordering::SeqCst),
            0,
            "locked row must not be dispatched"
        );
        assert_eq!(notif.gave_up_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn repeated_failures_eventually_notify_user() {
        // Drive the row through MAX_FAILED_ATTEMPTS failures by
        // resetting the backoff clock between ticks, and assert that
        // the GaveUp notification fires exactly once on the final tick.
        let pool = fresh_pool().await;
        let id = insert_scheduled(&pool, -60).await;
        let (disp, notif) = mock_pair(&pool, MockOutcome::Failure);

        // Tick MAX_FAILED_ATTEMPTS times, rewinding last_attempt_at far
        // enough between ticks to clear the backoff window. Loop bound
        // matches the const so the test stays in sync if the policy
        // ever changes.
        for i in 0..MAX_FAILED_ATTEMPTS {
            if i > 0 {
                sqlx::query("UPDATE post_history SET last_attempt_at = ? WHERE id = ?")
                    .bind((Utc::now() - chrono::Duration::hours(24)).to_rfc3339())
                    .bind(id)
                    .execute(&pool)
                    .await
                    .expect("rewind clock");
            }
            tick(&pool, &disp, &notif).await;
        }

        assert_eq!(
            disp.calls.load(Ordering::SeqCst),
            MAX_FAILED_ATTEMPTS as usize,
            "dispatcher must be called exactly MAX_FAILED_ATTEMPTS times"
        );
        let (status, attempts) = fetch_status(&pool, id).await;
        assert_eq!(status, "failed");
        assert_eq!(attempts, MAX_FAILED_ATTEMPTS);
        assert_eq!(
            notif.gave_up_calls.load(Ordering::SeqCst),
            1,
            "notification must fire exactly once when budget is exhausted"
        );
        assert_eq!(*notif.last_post_id.lock().unwrap(), Some(id));
    }

    #[tokio::test]
    async fn tick_respects_backoff_window_between_failures() {
        // After one failure, the row is in the 5-minute backoff window.
        // A second tick run immediately must NOT pick it up — the
        // dispatcher counter stays at 1.
        let pool = fresh_pool().await;
        let _id = insert_scheduled(&pool, -5).await;
        let (disp, notif) = mock_pair(&pool, MockOutcome::Failure);

        tick(&pool, &disp, &notif).await;
        assert_eq!(disp.calls.load(Ordering::SeqCst), 1);

        // Immediate re-tick. last_attempt_at was just set, so the
        // 5-min backoff window for failed_attempts=1 hasn't elapsed.
        tick(&pool, &disp, &notif).await;
        assert_eq!(
            disp.calls.load(Ordering::SeqCst),
            1,
            "row in backoff window must not be re-dispatched"
        );
    }

    #[tokio::test]
    async fn pre_check_token_expiry_blocks_expired_account() {
        // Insert an account with a past expiry and a draft attached.
        // pre_check_token_expiry must return Err — the dispatcher
        // production impl uses this to short-circuit the publish call.
        let pool = fresh_pool().await;
        let expired = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (provider, user_id, username, display_name, token_key,
                                   token_expires_at, updated_at)
             VALUES ('instagram', 'expired-user', 'expired', NULL, 'instagram:expired', ?,
                     datetime('now'))
             RETURNING id",
        )
        .bind(&expired)
        .fetch_one(&pool)
        .await
        .expect("insert expired account");

        let post_id: i64 = sqlx::query_scalar(
            "INSERT INTO post_history
                (network, caption, hashtags, status, created_at, scheduled_at, images, account_id)
             VALUES ('instagram', 'x', '[]', 'draft', ?, ?, '[]', ?)
             RETURNING id",
        )
        .bind(Utc::now().to_rfc3339())
        .bind((Utc::now() - chrono::Duration::minutes(5)).to_rfc3339())
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .expect("insert post");

        let post = crate::db::history::get_by_id(&pool, post_id)
            .await
            .expect("fetch post");
        let result = pre_check_token_expiry(&pool, &post).await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("expired"),
            "error must mention token expiry"
        );
    }

    #[tokio::test]
    async fn pre_check_token_expiry_allows_account_with_future_expiry() {
        let pool = fresh_pool().await;
        let future = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (provider, user_id, username, display_name, token_key,
                                   token_expires_at, updated_at)
             VALUES ('instagram', 'live-user', 'live', NULL, 'instagram:live', ?,
                     datetime('now'))
             RETURNING id",
        )
        .bind(&future)
        .fetch_one(&pool)
        .await
        .expect("insert live account");

        let post_id: i64 = sqlx::query_scalar(
            "INSERT INTO post_history
                (network, caption, hashtags, status, created_at, scheduled_at, images, account_id)
             VALUES ('instagram', 'x', '[]', 'draft', ?, ?, '[]', ?)
             RETURNING id",
        )
        .bind(Utc::now().to_rfc3339())
        .bind((Utc::now() - chrono::Duration::minutes(5)).to_rfc3339())
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .expect("insert post");

        let post = crate::db::history::get_by_id(&pool, post_id)
            .await
            .expect("fetch post");
        assert!(pre_check_token_expiry(&pool, &post).await.is_ok());
    }

    #[tokio::test]
    async fn pre_check_token_expiry_falls_open_for_legacy_post_with_no_account() {
        // Legacy row from before migration 013: account_id is NULL.
        // We can't look up an expiry, so the check falls open and the
        // publish call itself will surface any auth problem.
        let pool = fresh_pool().await;
        let post_id: i64 = sqlx::query_scalar(
            "INSERT INTO post_history
                (network, caption, hashtags, status, created_at, scheduled_at, images)
             VALUES ('instagram', 'x', '[]', 'draft', ?, ?, '[]')
             RETURNING id",
        )
        .bind(Utc::now().to_rfc3339())
        .bind((Utc::now() - chrono::Duration::minutes(5)).to_rfc3339())
        .fetch_one(&pool)
        .await
        .expect("insert legacy post");

        let post = crate::db::history::get_by_id(&pool, post_id)
            .await
            .expect("fetch post");
        assert!(pre_check_token_expiry(&pool, &post).await.is_ok());
    }
}
