/// OS-native secret storage for AI API keys + OAuth secrets.
///
/// ## Where the data lives
///
/// | OS      | Backend                              | Encrypted with        |
/// |---------|--------------------------------------|-----------------------|
/// | Windows | Credential Manager (Win32 wincred)   | DPAPI / user account  |
/// | macOS   | Keychain Services                    | User login keychain   |
/// | Linux   | Secret Service (D-Bus / libsecret)   | Per-user keyring      |
///
/// All three encrypt at rest with the OS user's credentials. A different
/// Windows user reading `api_keys.json` (the old plain-text format) used to
/// see every key — now they get an opaque blob they cannot decrypt.
///
/// ## Migration from plain-text JSON
///
/// On the first call to any function in this module after upgrading from
/// v0.1.0, if a legacy `{data_dir}/getpostcraft/api_keys.json` exists, every
/// key is moved into the OS secret store and the file is deleted. This is
/// idempotent: a re-run finds no file and is a no-op. See ADR-009 for the
/// security rationale.
///
/// ## load_all()
///
/// `keyring` has no "list entries" API — it abstracts platform stores that
/// don't all support enumeration. We pre-warm a cache by polling each
/// `KNOWN_PROVIDERS` entry. New providers must be registered there.
use keyring::Entry;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Service name namespacing all secrets owned by getpostcraft. The OS-level
/// secret stores key entries by `(service, account)`. We use a single service
/// name + the provider as account so entries stay grouped under our app in
/// e.g. macOS Keychain Access or Windows Credential Manager.
const SERVICE: &str = "app.getpostcraft.secrets";

/// Provider names we expect to find. `load_all` polls each of these to warm
/// the in-memory cache at startup. Adding a new provider? Append it here.
const KNOWN_PROVIDERS: &[&str] = &[
    "openrouter",
    "anthropic",
    "ollama",
    "instagram_client_secret",
    "linkedin_client_secret",
    "imgbb_api_key",
    // Argon2id PHC string for the Settings → Security gate. Not a "key"
    // in the API-credential sense but the keychain treats them the same
    // and we want load_all() to warm this entry alongside the others.
    "security_password_hash",
];

/// Path to the legacy plain-text JSON file. Existence triggers one-time migration.
fn legacy_keys_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("getpostcraft")
        .join("api_keys.json")
}

/// Open a keyring entry for a given provider. Returns Err if the OS secret
/// store is unavailable (e.g. headless Linux without a session bus).
fn entry(provider: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, provider).map_err(|e| format!("Cannot access OS keyring: {e}"))
}

/// Move legacy plain-text keys into the OS keyring, then delete the file.
/// Called automatically by every public function — guards against re-running
/// the migration on subsequent calls. Logs each step but never propagates
/// errors so app startup is never blocked by migration trouble.
fn migrate_legacy_if_present() {
    let path = legacy_keys_path();
    if !path.exists() {
        return;
    }
    log::info!("ai_keys: legacy api_keys.json detected — migrating to OS keyring");

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("ai_keys: cannot read legacy file ({e}) — skipping migration");
            return;
        }
    };
    let map: HashMap<String, String> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("ai_keys: legacy file is not valid JSON ({e}) — skipping migration");
            return;
        }
    };

    let mut migrated = 0usize;
    let mut failed = 0usize;
    for (provider, secret) in &map {
        match entry(provider).and_then(|e| {
            e.set_password(secret)
                .map_err(|err| format!("set_password: {err}"))
        }) {
            Ok(()) => migrated += 1,
            Err(e) => {
                failed += 1;
                log::warn!("ai_keys: migration of {provider} failed: {e}");
            }
        }
    }

    // Only delete the legacy file if EVERY key migrated successfully.
    // A partial migration with file deletion would lose secrets.
    if failed == 0 {
        if let Err(e) = fs::remove_file(&path) {
            log::warn!("ai_keys: keys migrated but legacy file delete failed: {e}");
        } else {
            log::info!("ai_keys: migrated {migrated} key(s) to OS keyring, legacy file removed");
        }
    } else {
        log::error!(
            "ai_keys: migration kept legacy file because {failed} key(s) failed; \
             will retry on next start"
        );
    }
}

pub fn save_key(provider: &str, key: &str) -> Result<(), String> {
    migrate_legacy_if_present();
    entry(provider)?
        .set_password(key)
        .map_err(|e| format!("Cannot save key for {provider}: {e}"))
}

pub fn get_key(provider: &str) -> Result<String, String> {
    migrate_legacy_if_present();
    let e = entry(provider)?;
    match e.get_password() {
        Ok(s) => Ok(s),
        // `keyring` returns NoEntry for missing — surface a friendlier message.
        Err(keyring::Error::NoEntry) => Err(format!("No key configured for provider: {provider}")),
        Err(err) => Err(format!("Cannot read key for {provider}: {err}")),
    }
}

pub fn delete_key(provider: &str) -> Result<(), String> {
    migrate_legacy_if_present();
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!("Cannot delete key for {provider}: {err}")),
    }
}

/// Load all known provider keys into a HashMap — used to warm the in-memory
/// cache at startup. Missing entries are silently skipped (a fresh install has
/// no keys yet, that's fine).
pub fn load_all() -> HashMap<String, String> {
    migrate_legacy_if_present();
    let mut out = HashMap::new();
    for provider in KNOWN_PROVIDERS {
        if let Ok(e) = entry(provider) {
            if let Ok(secret) = e.get_password() {
                out.insert((*provider).to_string(), secret);
            }
        }
    }
    out
}

pub fn has_key(provider: &str) -> bool {
    migrate_legacy_if_present();
    entry(provider)
        .ok()
        .and_then(|e| e.get_password().ok())
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialise tests that touch the same OS keyring service name.
    static KEYRING_LOCK: Mutex<()> = Mutex::new(());

    /// Each test uses a unique provider name so parallel suites (e.g. integration
    /// tests in another binary) don't collide. The keyring is process-shared.
    fn unique_provider() -> String {
        format!(
            "test_provider_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    /// Skip tests on machines without a usable keyring (headless CI Linux).
    /// Returns Some(provider) if writable, None to gracefully skip the test body.
    fn try_setup() -> Option<String> {
        let provider = unique_provider();
        match entry(&provider).and_then(|e| {
            e.set_password("probe")
                .map_err(|err| format!("probe: {err}"))
        }) {
            Ok(()) => {
                let _ = entry(&provider).and_then(|e| {
                    e.delete_credential()
                        .map_err(|err| format!("cleanup: {err}"))
                });
                Some(unique_provider())
            }
            Err(_) => None,
        }
    }

    #[test]
    fn save_and_get_key_roundtrip() {
        let _g = KEYRING_LOCK.lock().unwrap();
        let Some(provider) = try_setup() else { return };
        let key_value = "sk-test-1234567890abcdef";

        save_key(&provider, key_value).expect("save_key failed");
        let retrieved = get_key(&provider).expect("get_key failed");
        assert_eq!(retrieved, key_value);

        let _ = delete_key(&provider);
    }

    #[test]
    fn get_key_returns_err_for_unknown_provider() {
        let _g = KEYRING_LOCK.lock().unwrap();
        if try_setup().is_none() {
            return;
        }
        let result = get_key("nonexistent_provider_xyz_unique_123");
        assert!(result.is_err(), "must return Err for unknown provider");
        assert!(result.unwrap_err().contains("No key configured"));
    }

    #[test]
    fn delete_key_removes_entry() {
        let _g = KEYRING_LOCK.lock().unwrap();
        let Some(provider) = try_setup() else { return };
        save_key(&provider, "some_key_value").unwrap();
        assert!(has_key(&provider), "key must exist before delete");

        delete_key(&provider).unwrap();
        assert!(!has_key(&provider), "key must be gone after delete");
    }

    #[test]
    fn delete_nonexistent_key_is_ok() {
        let _g = KEYRING_LOCK.lock().unwrap();
        if try_setup().is_none() {
            return;
        }
        // Idempotent delete — never error on missing entries.
        let result = delete_key("definitely_not_a_real_provider_xyz");
        assert!(result.is_ok(), "deleting unknown key must not fail");
    }

    #[test]
    fn has_key_returns_false_for_unknown() {
        let _g = KEYRING_LOCK.lock().unwrap();
        if try_setup().is_none() {
            return;
        }
        assert!(!has_key("definitely_not_a_real_provider_xyz_456"));
    }

    #[test]
    fn overwrite_existing_key() {
        let _g = KEYRING_LOCK.lock().unwrap();
        let Some(provider) = try_setup() else { return };
        save_key(&provider, "first_value").unwrap();
        save_key(&provider, "second_value").unwrap();
        let result = get_key(&provider).unwrap();
        assert_eq!(result, "second_value", "overwrite must replace old value");

        let _ = delete_key(&provider);
    }

    #[test]
    fn known_providers_list_covers_documented_secrets() {
        // Regression guard: every provider name passed elsewhere in the codebase
        // must exist here so load_all warms it. If we add a new secret, we must
        // remember to add it to KNOWN_PROVIDERS.
        for required in [
            "openrouter",
            "anthropic",
            "ollama",
            "instagram_client_secret",
            "linkedin_client_secret",
            "imgbb_api_key",
            "security_password_hash",
        ] {
            assert!(
                KNOWN_PROVIDERS.contains(&required),
                "KNOWN_PROVIDERS must include {required}"
            );
        }
    }
}
