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
