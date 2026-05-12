use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::openrouter_pricing::{new_cache, PricingCache};
use crate::security_admin::SecurityAdminState;

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
    /// Live pricing snapshot from OpenRouter `/api/v1/models`. Empty until
    /// the first refresh runs; `network_rules::price_for` falls back to the
    /// static table when a model isn't here. Refreshed on startup and on
    /// manual user request from the AI usage panel.
    pub pricing_cache: PricingCache,
    /// Settings → Security gate state: in-RAM session + lockout tracker.
    /// Arc'd so async tasks holding a state guard don't block the lock
    /// across awaits — only the inner Mutexes are taken briefly.
    pub security_admin: Arc<SecurityAdminState>,
}

impl AppState {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            active_provider: Mutex::new(ActiveProvider {
                provider: "openrouter".to_string(),
                model: "anthropic/claude-sonnet-4.6".to_string(),
            }),
            key_cache: Mutex::new(HashMap::new()),
            db,
            pricing_cache: new_cache(),
            security_admin: Arc::new(SecurityAdminState::default()),
        }
    }
}
