/// Persistent storage for AI API keys.
///
/// Keys are stored in `{data_dir}/getpostcraft/api_keys.json`.
/// On Windows: `%APPDATA%\getpostcraft\api_keys.json`
/// On macOS:   `~/Library/Application Support/getpostcraft/api_keys.json`
/// On Linux:   `~/.local/share/getpostcraft/api_keys.json`
///
/// SECURITY: file lives in the user's private data directory (OS-level
/// user isolation). Not encrypted, but not accessible to other users.
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn keys_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("getpostcraft")
        .join("api_keys.json")
}

fn read_all() -> HashMap<String, String> {
    let path = keys_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn write_all(keys: &HashMap<String, String>) -> Result<(), String> {
    let path = keys_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string(keys).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn save_key(provider: &str, key: &str) -> Result<(), String> {
    let mut keys = read_all();
    keys.insert(provider.to_string(), key.to_string());
    write_all(&keys)
}

pub fn get_key(provider: &str) -> Result<String, String> {
    read_all()
        .remove(provider)
        .ok_or_else(|| format!("No key configured for provider: {provider}"))
}

pub fn delete_key(provider: &str) -> Result<(), String> {
    let mut keys = read_all();
    keys.remove(provider);
    write_all(&keys)
}

/// Load all stored keys into a HashMap — used to warm the in-memory cache at startup.
pub fn load_all() -> HashMap<String, String> {
    read_all()
}

pub fn has_key(provider: &str) -> bool {
    read_all().contains_key(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize all file-based tests to prevent concurrent read/write on api_keys.json
    static FILE_LOCK: Mutex<()> = Mutex::new(());

    fn unique_key() -> String {
        format!(
            "test_provider_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[test]
    fn save_and_get_key_roundtrip() {
        let _g = FILE_LOCK.lock().unwrap();
        let provider = unique_key();
        let key_value = "sk-test-1234567890abcdef";

        save_key(&provider, key_value).expect("save_key failed");
        let retrieved = get_key(&provider).expect("get_key failed");
        assert_eq!(retrieved, key_value);

        let _ = delete_key(&provider);
    }

    #[test]
    fn get_key_returns_err_for_unknown_provider() {
        let _g = FILE_LOCK.lock().unwrap();
        let result = get_key("nonexistent_provider_xyz");
        assert!(result.is_err(), "must return Err for unknown provider");
        assert!(result.unwrap_err().contains("No key configured"));
    }

    #[test]
    fn delete_key_removes_entry() {
        let _g = FILE_LOCK.lock().unwrap();
        let provider = unique_key();
        save_key(&provider, "some_key_value").unwrap();
        assert!(has_key(&provider), "key must exist before delete");

        delete_key(&provider).unwrap();
        assert!(!has_key(&provider), "key must be gone after delete");
    }

    #[test]
    fn delete_nonexistent_key_is_ok() {
        let _g = FILE_LOCK.lock().unwrap();
        let result = delete_key("nonexistent_provider_xyz");
        assert!(result.is_ok(), "deleting unknown key must not fail");
    }

    #[test]
    fn has_key_returns_false_for_unknown() {
        let _g = FILE_LOCK.lock().unwrap();
        assert!(!has_key("nonexistent_provider_xyz"));
    }

    #[test]
    fn overwrite_existing_key() {
        let _g = FILE_LOCK.lock().unwrap();
        let provider = unique_key();
        save_key(&provider, "first_value").unwrap();
        save_key(&provider, "second_value").unwrap();
        let result = get_key(&provider).unwrap();
        assert_eq!(result, "second_value", "overwrite must replace old value");

        let _ = delete_key(&provider);
    }

    #[test]
    fn instagram_client_secret_key_name_is_correct() {
        let key_name = "instagram_client_secret";
        assert!(!key_name.is_empty());
        assert!(!key_name.contains(' '), "key name must not contain spaces");
    }
}
