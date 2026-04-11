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

#[allow(dead_code)]
pub fn has_key(provider: &str) -> bool {
    read_all().contains_key(provider)
}
