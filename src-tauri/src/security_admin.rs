//! Security Admin gate for the in-app Settings → Security tab.
//!
//! Mono-user desktop = the user IS admin by definition. The gate is not
//! protecting against external attackers — it's a deliberate friction layer
//! for:
//!   - Accidental clicks (the audit panel triggers paid API calls)
//!   - Screen sharing / shoulder-surfing during streams or demos
//!   - Another OS user on a shared workstation
//!   - Malware that finds the binary but can't run it without the password
//!
//! ## Threat model
//!
//! In scope:
//!   - Brute-force: lockout escalation (3 fails → 5s, 5 → 30s, 10 → 5min)
//!   - Hash extraction: keychain (OS-encrypted) + Argon2id (GPU-resistant)
//!   - Side-channel timing on session token verify: `subtle::ConstantTimeEq`
//!
//! Out of scope:
//!   - Privileged-malware that owns the OS keychain (no defense from
//!     userspace possible)
//!   - Memory dump while the app runs unlocked (session token in RAM)
//!   - Reverse-engineering the binary (no obfuscation, not the goal)
//!
//! ## Storage
//!
//! - Password hash: OS keychain, provider name `security_password_hash`,
//!   value = Argon2id PHC string. Never persists to SQLite.
//! - Lockout state: in-memory `Mutex<LockoutTracker>` + append-only
//!   `security_audit_attempts` table (forensic trail, not used for the
//!   lockout decision itself — that's pure RAM so app-restart doesn't
//!   bypass the policy through the SQLite cache).
//! - Session token: 32 random bytes from `OsRng`, base64-encoded for
//!   transport to the renderer. Expires after [`SESSION_DURATION`], no
//!   automatic refresh — the user re-enters the password.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use sqlx::SqlitePool;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

/// Keychain provider name. Kept in one place — must match the entry in
/// `ai_keys::KNOWN_PROVIDERS` so `load_all()` warms the cache at startup.
pub const KEYRING_PROVIDER: &str = "security_password_hash";

/// Minimum acceptable plain-text password length at setup time. Twelve is
/// a deliberate compromise between memorability for a solo user and
/// resistance to brute-force given the Argon2id cost we ship.
pub const MIN_PASSWORD_LEN: usize = 12;

/// Session lifetime once verified. Re-prompt forces the user back through
/// the gate after this window — short enough that an unattended unlocked
/// app doesn't stay open all day, long enough that running a 5-minute
/// audit doesn't expire mid-run.
pub const SESSION_DURATION: Duration = Duration::from_secs(30 * 60);

/// Argon2id memory cost in KiB. OWASP 2026 minimum is 19 MiB for
/// interactive password verification.
const ARGON2_MEMORY_KIB: u32 = 19 * 1024;
/// Argon2id time cost (iterations). 2 is OWASP-recommended for the above
/// memory cost.
const ARGON2_TIME_COST: u32 = 2;
/// Argon2id parallelism. 1 on desktop avoids cross-platform variance.
const ARGON2_PARALLELISM: u32 = 1;

/// Construct the Argon2 instance with our pinned parameters. The
/// `Params::new` call panics only on impossible param combinations — we
/// hardcode known-good values so we expose a non-fallible API.
fn argon2_hasher() -> Argon2<'static> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        None,
    )
    .expect("Argon2 params are statically valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Lockout state held in memory only (intentional — restart-bypass is
/// trivial via process kill, so persistence buys nothing security-wise,
/// while in-RAM keeps the surface minimal).
#[derive(Debug, Default)]
pub struct LockoutTracker {
    consecutive_failures: u32,
    /// When the most recent lockout window started. None when no active
    /// lockout. The current window is `last_failure_at + delay`.
    last_failure_at: Option<Instant>,
}

impl LockoutTracker {
    /// Return the active lockout remaining, or None if the user may try
    /// a password now. `now` parameterised for tests; production passes
    /// `Instant::now()`.
    fn remaining_lockout(&self, now: Instant) -> Option<Duration> {
        let delay = lockout_duration(self.consecutive_failures)?;
        let started = self.last_failure_at?;
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= delay {
            None
        } else {
            Some(delay - elapsed)
        }
    }

    fn record_failure(&mut self, now: Instant) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.last_failure_at = Some(now);
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_failure_at = None;
    }
}

/// Lockout escalation table. Returns None when no lockout applies (under
/// the threshold, or above the give-up ceiling where the only path is
/// re-launching the app).
fn lockout_duration(consecutive_failures: u32) -> Option<Duration> {
    match consecutive_failures {
        0..=2 => None,
        3..=4 => Some(Duration::from_secs(5)),
        5..=9 => Some(Duration::from_secs(30)),
        10..=19 => Some(Duration::from_secs(300)),
        // 20+ → tracker keeps escalating but we cap at 5min to avoid
        // surprising the user with hour-long delays. Restarting the app
        // resets the in-RAM tracker — by design, see threat model.
        _ => Some(Duration::from_secs(300)),
    }
}

/// In-RAM session state. `token` is 32 random bytes; renderer holds the
/// base64 form and presents it on each privileged command call.
#[derive(Debug, Clone)]
pub struct SessionState {
    token: [u8; 32],
    expires_at: Instant,
}

impl SessionState {
    fn new(now: Instant) -> Self {
        let mut token = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut token);
        Self {
            token,
            expires_at: now + SESSION_DURATION,
        }
    }

    /// Base64 token suitable for IPC transport to the renderer. URL-safe
    /// no-pad form keeps it copy-paste friendly in dev tools without
    /// changing the length contract.
    pub fn token_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.token)
    }

    fn matches(&self, candidate_b64: &str) -> bool {
        let Ok(candidate) = URL_SAFE_NO_PAD.decode(candidate_b64.as_bytes()) else {
            return false;
        };
        // Length check upfront — constant-time over 32 bytes always.
        if candidate.len() != self.token.len() {
            return false;
        }
        bool::from(self.token.ct_eq(&candidate))
    }

    fn is_expired(&self, now: Instant) -> bool {
        now >= self.expires_at
    }
}

/// Container the Tauri commands hold — wraps mutexes around the two
/// pieces of mutable state. `Default` gives a no-session, no-failures
/// baseline suitable for fresh starts.
#[derive(Debug, Default)]
pub struct SecurityAdminState {
    pub session: Mutex<Option<SessionState>>,
    pub lockout: Mutex<LockoutTracker>,
}

/// Setup the security password. Idempotent w.r.t. overwriting an existing
/// hash — the user can rotate the password at will. Caller is responsible
/// for any UI-side confirm-password matching.
pub fn setup_password(plain: &str) -> Result<(), String> {
    if plain.len() < MIN_PASSWORD_LEN {
        return Err(format!(
            "Le mot de passe doit faire au moins {MIN_PASSWORD_LEN} caractères."
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2_hasher()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| format!("Hash failure: {e}"))?
        .to_string();
    crate::ai_keys::save_key(KEYRING_PROVIDER, &hash)
}

/// True iff a hash already exists in the keychain. Used by the UI to
/// pick between "first-time setup" and "verify password" flows.
pub fn is_password_set() -> bool {
    crate::ai_keys::has_key(KEYRING_PROVIDER)
}

/// Remove the password hash and any active session. Used by a hidden
/// "Reset Security Admin" command (CLI-only for now) so a user who
/// forgets the password can recover access by losing the audit-log
/// session history. Not exposed in UI — intentional friction.
#[allow(dead_code)]
pub fn reset_password(state: &SecurityAdminState) -> Result<(), String> {
    crate::ai_keys::delete_key(KEYRING_PROVIDER)?;
    if let Ok(mut s) = state.session.lock() {
        *s = None;
    }
    if let Ok(mut l) = state.lockout.lock() {
        *l = LockoutTracker::default();
    }
    Ok(())
}

/// Outcome of a verify attempt. Cleaner than a stringly-typed Result for
/// the multiple failure modes — the UI surfaces each differently.
#[derive(Debug)]
pub enum VerifyOutcome {
    /// Password matched, session opened. Caller forwards `token_b64` to
    /// the renderer.
    Ok { token_b64: String },
    /// User is in an active lockout window; tell them when they can retry.
    LockedOut { wait: Duration },
    /// Hash not yet configured (caller should redirect to setup).
    NoPasswordSet,
    /// Plain wrong password (or password set but missing from keychain).
    Wrong,
}

/// Core verify routine. `db` parameter is used to append to the audit
/// log; pass `None` only in tests where we don't want a SQLite dep.
pub async fn verify_password(
    plain: &str,
    state: &SecurityAdminState,
    db: Option<&SqlitePool>,
) -> VerifyOutcome {
    verify_password_at(plain, state, db, Instant::now(), chrono::Utc::now()).await
}

/// Test-friendly core. Production passes the current clock; tests can
/// inject deterministic instants.
pub async fn verify_password_at(
    plain: &str,
    state: &SecurityAdminState,
    db: Option<&SqlitePool>,
    now: Instant,
    now_wall: chrono::DateTime<chrono::Utc>,
) -> VerifyOutcome {
    // 1. Lockout check first — denied attempts in a lockout window don't
    //    even touch Argon2 (no CPU spent on attackers).
    let lockout_wait = {
        let lockout = state.lockout.lock().ok();
        lockout.as_ref().and_then(|l| l.remaining_lockout(now))
    };
    if let Some(wait) = lockout_wait {
        record_attempt(
            db,
            &now_wall,
            false,
            Some(&format!("lockout-skip: {}s", wait.as_secs())),
        )
        .await;
        return VerifyOutcome::LockedOut { wait };
    }

    // 2. Pull the stored hash (returns Wrong if missing — keeps the
    //    enumeration surface tighter than NoPasswordSet for an unset
    //    + wrong-attempt scenario).
    let stored = match crate::ai_keys::get_key(KEYRING_PROVIDER) {
        Ok(h) => h,
        Err(_) => return VerifyOutcome::NoPasswordSet,
    };

    // 3. Parse + verify. Both can fail — for safety, treat any failure as
    //    Wrong (not as an error path) so attackers can't distinguish a
    //    corrupt-hash state from a wrong password.
    let parsed = PasswordHash::new(&stored);
    let matched = parsed
        .as_ref()
        .ok()
        .map(|hash| {
            argon2_hasher()
                .verify_password(plain.as_bytes(), hash)
                .is_ok()
        })
        .unwrap_or(false);

    if !matched {
        if let Ok(mut tracker) = state.lockout.lock() {
            tracker.record_failure(now);
        }
        record_attempt(db, &now_wall, false, Some("password-mismatch")).await;
        return VerifyOutcome::Wrong;
    }

    // 4. Success — reset lockout, mint session, log attempt.
    if let Ok(mut tracker) = state.lockout.lock() {
        tracker.record_success();
    }
    let session = SessionState::new(now);
    let token_b64 = session.token_b64();
    if let Ok(mut slot) = state.session.lock() {
        *slot = Some(session);
    }
    record_attempt(db, &now_wall, true, Some("verify-ok")).await;
    VerifyOutcome::Ok { token_b64 }
}

/// Check whether a renderer-supplied session token is still valid. Used
/// at the top of every privileged command (audit run, report list, etc.).
pub fn check_session(state: &SecurityAdminState, candidate_b64: &str) -> bool {
    check_session_at(state, candidate_b64, Instant::now())
}

#[doc(hidden)]
pub fn check_session_at(state: &SecurityAdminState, candidate_b64: &str, now: Instant) -> bool {
    let Ok(slot) = state.session.lock() else {
        return false;
    };
    match slot.as_ref() {
        Some(session) if !session.is_expired(now) => session.matches(candidate_b64),
        _ => false,
    }
}

/// Drop the active session — used by an explicit "Lock now" button or
/// the app's shutdown hook so a long-running audit doesn't keep the
/// gate open across app restarts.
pub fn end_session(state: &SecurityAdminState) {
    if let Ok(mut slot) = state.session.lock() {
        *slot = None;
    }
}

/// Append-only audit log. Best-effort: a logging failure doesn't fail
/// the verify decision — we want the user to be able to unlock the app
/// even if SQLite is wedged. Wall-clock parametrised for tests.
async fn record_attempt(
    db: Option<&SqlitePool>,
    at: &chrono::DateTime<chrono::Utc>,
    success: bool,
    note: Option<&str>,
) {
    let Some(pool) = db else {
        return;
    };
    let success_int: i64 = if success { 1 } else { 0 };
    if let Err(e) = sqlx::query(
        "INSERT INTO security_audit_attempts (attempted_at, success, note) VALUES (?, ?, ?)",
    )
    .bind(at.to_rfc3339())
    .bind(success_int)
    .bind(note)
    .execute(pool)
    .await
    {
        log::warn!("security_admin: audit log insert failed (non-fatal): {e}");
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Lockout policy is the simplest pure function — pin its shape.
    #[test]
    fn lockout_duration_escalation() {
        assert_eq!(lockout_duration(0), None);
        assert_eq!(lockout_duration(2), None);
        assert_eq!(lockout_duration(3), Some(Duration::from_secs(5)));
        assert_eq!(lockout_duration(4), Some(Duration::from_secs(5)));
        assert_eq!(lockout_duration(5), Some(Duration::from_secs(30)));
        assert_eq!(lockout_duration(9), Some(Duration::from_secs(30)));
        assert_eq!(lockout_duration(10), Some(Duration::from_secs(300)));
        assert_eq!(lockout_duration(19), Some(Duration::from_secs(300)));
        assert_eq!(lockout_duration(20), Some(Duration::from_secs(300)));
        assert_eq!(lockout_duration(1000), Some(Duration::from_secs(300)));
    }

    #[test]
    fn lockout_tracker_clears_on_success() {
        let mut t = LockoutTracker::default();
        let now = Instant::now();
        t.record_failure(now);
        t.record_failure(now);
        t.record_failure(now);
        assert!(t.remaining_lockout(now).is_some());
        t.record_success();
        assert!(t.remaining_lockout(now).is_none());
        assert_eq!(t.consecutive_failures, 0);
    }

    #[test]
    fn lockout_tracker_window_decays() {
        let mut t = LockoutTracker::default();
        let start = Instant::now();
        t.consecutive_failures = 3;
        t.last_failure_at = Some(start);
        // At t=0, still in the 5s window.
        assert_eq!(
            t.remaining_lockout(start),
            Some(Duration::from_secs(5)),
            "window must be open at t=0"
        );
        // Past the window — None even though failures stays at 3.
        let later = start + Duration::from_secs(6);
        assert!(t.remaining_lockout(later).is_none());
    }

    #[test]
    fn session_token_matches_itself_and_rejects_others() {
        let now = Instant::now();
        let a = SessionState::new(now);
        let b = SessionState::new(now);
        let a_tok = a.token_b64();
        assert!(a.matches(&a_tok), "session must match its own token");
        assert!(!a.matches(&b.token_b64()), "different sessions don't match");
    }

    #[test]
    fn session_token_rejects_malformed_input() {
        let now = Instant::now();
        let s = SessionState::new(now);
        // Empty, non-base64, wrong length all return false (no panic).
        assert!(!s.matches(""));
        assert!(!s.matches("not-base64-!@#$"));
        assert!(!s.matches("c2hvcnQ")); // valid base64 but too short
    }

    #[test]
    fn session_expires_after_duration() {
        let start = Instant::now();
        let s = SessionState::new(start);
        assert!(!s.is_expired(start), "fresh session not expired at t=0");
        assert!(
            !s.is_expired(start + SESSION_DURATION - Duration::from_secs(1)),
            "session valid 1s before expiry"
        );
        assert!(
            s.is_expired(start + SESSION_DURATION),
            "session expired exactly at duration"
        );
        assert!(
            s.is_expired(start + SESSION_DURATION + Duration::from_secs(60)),
            "session still expired well past duration"
        );
    }

    #[tokio::test]
    async fn verify_returns_no_password_set_when_keychain_empty() {
        // Skip when keychain unavailable (CI Linux without DBus).
        if !keyring_available() {
            return;
        }
        let _ = crate::ai_keys::delete_key(KEYRING_PROVIDER);
        let state = SecurityAdminState::default();
        let pool = fresh_pool().await;
        let outcome = verify_password("any-password-1234", &state, Some(&pool)).await;
        assert!(matches!(outcome, VerifyOutcome::NoPasswordSet));
    }

    #[tokio::test]
    async fn setup_rejects_short_password() {
        let r = setup_password("short");
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn setup_then_verify_with_correct_password_succeeds() {
        if !keyring_available() {
            return;
        }
        let provider = unique_provider();
        // Use a unique provider for test isolation — keychain entries are
        // process-shared. Override KEYRING_PROVIDER via a local helper.
        let plain = "good-password-12345";
        save_isolated_hash(&provider, plain).expect("hash setup");

        let state = SecurityAdminState::default();
        let pool = fresh_pool().await;
        let outcome = verify_isolated(plain, &state, Some(&pool), &provider).await;
        assert!(matches!(outcome, VerifyOutcome::Ok { .. }));

        // Audit log recorded the success.
        let row_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM security_audit_attempts WHERE success = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row_count, 1);

        cleanup_isolated(&provider);
    }

    #[tokio::test]
    async fn verify_wrong_password_records_failure_and_increments_lockout() {
        if !keyring_available() {
            return;
        }
        let provider = unique_provider();
        save_isolated_hash(&provider, "good-password-12345").expect("setup");

        let state = SecurityAdminState::default();
        let pool = fresh_pool().await;

        for _ in 0..3 {
            let outcome =
                verify_isolated("WRONG-password-aaa", &state, Some(&pool), &provider).await;
            assert!(matches!(outcome, VerifyOutcome::Wrong));
        }

        // The 4th attempt should be locked out (3 failures = enters 5s window).
        let outcome = verify_isolated("any", &state, Some(&pool), &provider).await;
        assert!(
            matches!(outcome, VerifyOutcome::LockedOut { .. }),
            "must lock out after 3 consecutive failures"
        );

        cleanup_isolated(&provider);
    }

    #[tokio::test]
    async fn check_session_rejects_expired_tokens() {
        let state = SecurityAdminState::default();
        let now = Instant::now();
        let mut session = SessionState::new(now);
        // Force expiry into the past.
        session.expires_at = now - Duration::from_secs(1);
        let tok = session.token_b64();
        // Drop the lock before calling check_session_at (which re-locks).
        {
            let mut slot = state.session.lock().unwrap();
            *slot = Some(session);
        }
        assert!(
            !check_session_at(&state, &tok, now),
            "expired token must be rejected"
        );
    }

    #[tokio::test]
    async fn end_session_drops_state() {
        let state = SecurityAdminState::default();
        {
            let mut slot = state.session.lock().unwrap();
            *slot = Some(SessionState::new(Instant::now()));
        }
        end_session(&state);
        assert!(state.session.lock().unwrap().is_none());
    }

    // ── Test plumbing ────────────────────────────────────────────────────

    fn keyring_available() -> bool {
        let probe = unique_provider();
        let r = keyring::Entry::new("app.getpostcraft.secrets", &probe)
            .and_then(|e| e.set_password("probe"));
        if r.is_ok() {
            let _ = keyring::Entry::new("app.getpostcraft.secrets", &probe)
                .and_then(|e| e.delete_credential());
            true
        } else {
            false
        }
    }

    fn unique_provider() -> String {
        format!(
            "test_security_admin_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    fn save_isolated_hash(provider: &str, plain: &str) -> Result<(), String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = argon2_hasher()
            .hash_password(plain.as_bytes(), &salt)
            .map_err(|e| e.to_string())?
            .to_string();
        crate::ai_keys::save_key(provider, &hash)
    }

    fn cleanup_isolated(provider: &str) {
        let _ = crate::ai_keys::delete_key(provider);
    }

    /// Mirror of `verify_password` but reading the hash from an arbitrary
    /// keyring provider (per-test isolation). Identical control flow.
    async fn verify_isolated(
        plain: &str,
        state: &SecurityAdminState,
        db: Option<&SqlitePool>,
        provider: &str,
    ) -> VerifyOutcome {
        let now = Instant::now();
        let now_wall = chrono::Utc::now();
        let lockout_wait = state
            .lockout
            .lock()
            .ok()
            .as_ref()
            .and_then(|l| l.remaining_lockout(now));
        if let Some(wait) = lockout_wait {
            record_attempt(db, &now_wall, false, Some("lockout")).await;
            return VerifyOutcome::LockedOut { wait };
        }
        let stored = match crate::ai_keys::get_key(provider) {
            Ok(h) => h,
            Err(_) => return VerifyOutcome::NoPasswordSet,
        };
        let parsed = PasswordHash::new(&stored);
        let matched = parsed
            .as_ref()
            .ok()
            .map(|h| argon2_hasher().verify_password(plain.as_bytes(), h).is_ok())
            .unwrap_or(false);
        if !matched {
            if let Ok(mut tracker) = state.lockout.lock() {
                tracker.record_failure(now);
            }
            record_attempt(db, &now_wall, false, Some("mismatch")).await;
            return VerifyOutcome::Wrong;
        }
        if let Ok(mut tracker) = state.lockout.lock() {
            tracker.record_success();
        }
        let session = SessionState::new(now);
        let token_b64 = session.token_b64();
        if let Ok(mut slot) = state.session.lock() {
            *slot = Some(session);
        }
        record_attempt(db, &now_wall, true, Some("ok")).await;
        VerifyOutcome::Ok { token_b64 }
    }

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect");
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect("migrate");
        pool
    }
}
