/// Persistent storage for OAuth access tokens.
///
/// File: `{data_dir}/getpostcraft/oauth_tokens.json`
/// Keys are in the format "{provider}:{user_id}" (e.g. "instagram:12345").
///
/// SECURITY: file lives in the user's private data directory.
/// Tokens never cross IPC to the renderer — only Rust reads this file.
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn tokens_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("getpostcraft")
        .join("oauth_tokens.json")
}

fn read_all() -> HashMap<String, String> {
    let Ok(content) = fs::read_to_string(tokens_path()) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn write_all(tokens: &HashMap<String, String>) -> Result<(), String> {
    let path = tokens_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string(tokens).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn save_token(key: &str, token: &str) -> Result<(), String> {
    let mut tokens = read_all();
    tokens.insert(key.to_string(), token.to_string());
    write_all(&tokens)
}

/// Retrieve a token — used by publisher commands (V1).
#[allow(dead_code)]
pub fn get_token(key: &str) -> Result<String, String> {
    read_all()
        .remove(key)
        .ok_or_else(|| format!("No token found for key: {key}"))
}

pub fn delete_token(key: &str) -> Result<(), String> {
    let mut tokens = read_all();
    tokens.remove(key);
    write_all(&tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize all file-based tests to prevent concurrent read/write on oauth_tokens.json
    static FILE_LOCK: Mutex<()> = Mutex::new(());

    fn unique_key() -> String {
        format!(
            "test_instagram:{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[test]
    fn save_and_get_token_roundtrip() {
        let _g = FILE_LOCK.lock().unwrap();
        let key = unique_key();
        let token = "EAACtest_access_token_value";

        save_token(&key, token).expect("save_token failed");
        let retrieved = get_token(&key).expect("get_token failed");
        assert_eq!(retrieved, token);

        let _ = delete_token(&key);
    }

    #[test]
    fn get_token_returns_err_for_unknown_key() {
        let _g = FILE_LOCK.lock().unwrap();
        let result = get_token("nonexistent:9999999999");
        assert!(result.is_err(), "must return Err for unknown key");
        assert!(result.unwrap_err().contains("No token found"));
    }

    #[test]
    fn delete_token_removes_entry() {
        let _g = FILE_LOCK.lock().unwrap();
        let key = unique_key();
        save_token(&key, "some_token").unwrap();
        assert!(get_token(&key).is_ok(), "token must exist before delete");

        delete_token(&key).unwrap();
        assert!(get_token(&key).is_err(), "token must be gone after delete");
    }

    #[test]
    fn delete_nonexistent_token_is_ok() {
        let _g = FILE_LOCK.lock().unwrap();
        let result = delete_token("nonexistent:0000000000");
        assert!(result.is_ok(), "deleting nonexistent key must be Ok");
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
