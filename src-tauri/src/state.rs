use std::collections::HashMap;
use std::sync::Mutex;
use sqlx::SqlitePool;

pub struct ActiveProvider {
    /// "openrouter" | "anthropic" | "ollama"
    pub provider: String,
    pub model: String,
}

pub struct AppState {
    pub active_provider: Mutex<ActiveProvider>,
    /// In-memory cache of validated API keys — avoids keychain re-reads per call.
    /// Populated by save_ai_key, cleared by delete_ai_key.
    pub key_cache: Mutex<HashMap<String, String>>,
    /// SQLite connection pool — initialized async at startup.
    pub db: SqlitePool,
}

impl AppState {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            active_provider: Mutex::new(ActiveProvider {
                provider: "openrouter".to_string(),
                model: "anthropic/claude-3-5-haiku".to_string(),
            }),
            key_cache: Mutex::new(HashMap::new()),
            db,
        }
    }
}
