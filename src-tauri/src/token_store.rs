/// OS-native secret storage for OAuth access tokens.
///
/// ## Where the data lives
///
/// | OS      | Backend                              | Encrypted with        |
/// |---------|--------------------------------------|-----------------------|
/// | Windows | Credential Manager (Win32 wincred)   | DPAPI / user account  |
/// | macOS   | Keychain Services                    | User login keychain   |
/// | Linux   | Secret Service (D-Bus / libsecret)   | Per-user keyring      |
///
/// ## Why a separate SERVICE from `ai_keys.rs`
///
/// AI provider keys and OAuth tokens are conceptually distinct:
///   - Different lifecycles (AI keys: user-pasted, long-lived; OAuth tokens:
///     issued by provider, can be rotated).
///   - Different blast radius if leaked (AI key = pay-per-call cost; OAuth
///     token = ability to publish on someone's social account).
///   - Cleaner separation in the OS credential manager UI for the user
///     (Windows Credential Manager, macOS Keychain Access).
///
/// ## Migration from plain-text JSON
///
/// PR-A (#18) migrated AI keys to the keyring but left OAuth tokens in
/// `oauth_tokens.json` — a plain-text file readable by any process running
/// as the same OS user. PR-X closes that gap. On the first call after
/// upgrading, if `{data_dir}/getpostcraft/oauth_tokens.json` exists, every
/// entry is moved into the keyring and the file is deleted. Idempotent.
///
/// ## Key format
///
/// Keys follow `"{provider}:{user_id}"` (e.g. `"instagram:12345"`). The DB
/// `accounts.token_key` column is the source of truth — we never enumerate
/// the keyring (it has no list API on most platforms).
///
/// ## SECURITY
///
/// Tokens never cross IPC to the renderer. Only Rust reads them.
/// `Display`/`Debug` of internal values is intentionally avoided — error
/// messages reference the *key* (provider + user_id) but never the token.
use keyring::Entry;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Service name for OAuth tokens. Distinct from `app.getpostcraft.secrets`
/// (used by `ai_keys.rs`) so the two stay clearly separated in the OS
/// credential manager.
const SERVICE: &str = "app.getpostcraft.oauth-tokens";

/// Path to the legacy plain-text JSON file. Existence triggers one-time migration.
fn legacy_tokens_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("getpostcraft")
        .join("oauth_tokens.json")
}

fn entry(key: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, key).map_err(|e| format!("Cannot access OS keyring: {e}"))
}

/// Move legacy plain-text OAuth tokens into the keyring, then delete the file.
/// Called by every public function — guards against re-running. Failures are
/// logged but never propagated so app startup is not blocked.
fn migrate_legacy_if_present() {
    let path = legacy_tokens_path();
    if !path.exists() {
        return;
    }
    log::info!("token_store: legacy oauth_tokens.json detected — migrating to OS keyring");

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("token_store: cannot read legacy file ({e}) — skipping migration");
            return;
        }
    };
    let map: HashMap<String, String> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("token_store: legacy file is not valid JSON ({e}) — skipping migration");
            return;
        }
    };

    let mut migrated = 0usize;
    let mut failed = 0usize;
    for (key, token) in &map {
        match entry(key).and_then(|e| {
            e.set_password(token)
                .map_err(|err| format!("set_password: {err}"))
        }) {
            Ok(()) => migrated += 1,
            Err(e) => {
                failed += 1;
                // Note: we deliberately do NOT log the token value, only the key.
                log::warn!("token_store: migration of {key} failed: {e}");
            }
        }
    }

    // Only delete the legacy file if EVERY token migrated successfully.
    // A partial migration with file deletion would lose tokens.
    if failed == 0 {
        if let Err(e) = fs::remove_file(&path) {
            log::warn!("token_store: tokens migrated but legacy file delete failed: {e}");
        } else {
            log::info!(
                "token_store: migrated {migrated} token(s) to OS keyring, legacy file removed"
            );
        }
    } else {
        log::error!(
            "token_store: migration kept legacy file because {failed} token(s) failed; \
             will retry on next start"
        );
    }
}

pub fn save_token(key: &str, token: &str) -> Result<(), String> {
    migrate_legacy_if_present();
    entry(key)?
        .set_password(token)
        // Never include the token value in the error message — it would land
        // in logs and crash reports.
        .map_err(|e| format!("Cannot save token for {key}: {e}"))
}

/// Retrieve a token. Used by publisher commands.
pub fn get_token(key: &str) -> Result<String, String> {
    migrate_legacy_if_present();
    let e = entry(key)?;
    match e.get_password() {
        Ok(s) => Ok(s),
        Err(keyring::Error::NoEntry) => Err(format!("No token found for key: {key}")),
        Err(err) => Err(format!("Cannot read token for {key}: {err}")),
    }
}

pub fn delete_token(key: &str) -> Result<(), String> {
    migrate_legacy_if_present();
    match entry(key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!("Cannot delete token for {key}: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialise tests touching the same OS keyring service.
    static KEYRING_LOCK: Mutex<()> = Mutex::new(());

    fn unique_key() -> String {
        format!(
            "test_instagram:{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    /// Skip tests on machines without a usable keyring (headless CI Linux).
    fn try_setup() -> Option<String> {
        let key = unique_key();
        match entry(&key).and_then(|e| {
            e.set_password("probe")
                .map_err(|err| format!("probe: {err}"))
        }) {
            Ok(()) => {
                let _ = entry(&key).and_then(|e| {
                    e.delete_credential()
                        .map_err(|err| format!("cleanup: {err}"))
                });
                Some(unique_key())
            }
            Err(_) => None,
        }
    }

    #[test]
    fn save_and_get_token_roundtrip() {
        let _g = KEYRING_LOCK.lock().unwrap();
        let Some(key) = try_setup() else { return };
        let token = "EAACtest_access_token_value";

        save_token(&key, token).expect("save_token failed");
        let retrieved = get_token(&key).expect("get_token failed");
        assert_eq!(retrieved, token);

        let _ = delete_token(&key);
    }

    #[test]
    fn get_token_returns_err_for_unknown_key() {
        let _g = KEYRING_LOCK.lock().unwrap();
        if try_setup().is_none() {
            return;
        }
        let result = get_token("nonexistent:9999999999");
        assert!(result.is_err(), "must return Err for unknown key");
        assert!(result.unwrap_err().contains("No token found"));
    }

    #[test]
    fn delete_token_removes_entry() {
        let _g = KEYRING_LOCK.lock().unwrap();
        let Some(key) = try_setup() else { return };
        save_token(&key, "some_token").unwrap();
        assert!(get_token(&key).is_ok(), "token must exist before delete");

        delete_token(&key).unwrap();
        assert!(get_token(&key).is_err(), "token must be gone after delete");
    }

    #[test]
    fn delete_nonexistent_token_is_ok() {
        let _g = KEYRING_LOCK.lock().unwrap();
        if try_setup().is_none() {
            return;
        }
        let result = delete_token("nonexistent:0000000000");
        assert!(result.is_ok(), "deleting nonexistent key must be Ok");
    }

    #[test]
    fn overwrite_existing_token() {
        let _g = KEYRING_LOCK.lock().unwrap();
        let Some(key) = try_setup() else { return };
        save_token(&key, "first_token").unwrap();
        save_token(&key, "second_token").unwrap();
        let result = get_token(&key).unwrap();
        assert_eq!(result, "second_token", "overwrite must replace old value");

        let _ = delete_token(&key);
    }

    #[test]
    fn token_key_format_is_provider_colon_user_id() {
        let key = "instagram:12345678";
        assert!(key.contains(':'), "must follow provider:user_id format");
        let parts: Vec<&str> = key.splitn(2, ':').collect();
        assert_eq!(parts[0], "instagram");
        assert_eq!(parts[1], "12345678");
    }
}
