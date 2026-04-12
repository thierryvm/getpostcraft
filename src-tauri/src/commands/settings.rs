use crate::state::AppState;
use serde::Serialize;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct KeyValidationResult {
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiKeyStatus {
    pub configured: bool,
    /// "••••••••xK3p" — never the full key
    pub masked: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub provider: String,
    pub model: String,
}

// ── Private validation helpers ────────────────────────────────────────────────

async fn build_reqwest() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())
}

async fn validate_openrouter(api_key: &str) -> Result<(), String> {
    let resp = build_reqwest()
        .await?
        .get("https://openrouter.ai/api/v1/auth/key")
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|e| format!("Réseau : {e}"))?;

    match resp.status().as_u16() {
        200 => Ok(()),
        401 | 403 => Err("Clé OpenRouter invalide".to_string()),
        s => Err(format!("Erreur OpenRouter (HTTP {s})")),
    }
}

async fn validate_anthropic(api_key: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 1,
        "messages": [{ "role": "user", "content": "hi" }]
    });

    let resp = build_reqwest()
        .await?
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Réseau : {e}"))?;

    match resp.status().as_u16() {
        200 | 429 => Ok(()),
        401 | 403 => Err("Clé Anthropic invalide".to_string()),
        s => Err(format!("Erreur Anthropic (HTTP {s})")),
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Validates key format + calls provider API, stores in keychain on success.
/// SECURITY: key is never returned to renderer.
#[tauri::command]
pub async fn save_ai_key(
    state: tauri::State<'_, AppState>,
    provider: String,
    key: String,
) -> Result<KeyValidationResult, String> {
    let prefix = match provider.as_str() {
        "anthropic" => Some("sk-ant-"),
        "openrouter" => Some("sk-or-"),
        _ => return Err(format!("Provider inconnu : {provider}")),
    };

    if let Some(p) = prefix {
        if !key.starts_with(p) {
            return Ok(KeyValidationResult {
                valid: false,
                error: Some(format!("La clé doit commencer par « {p} »")),
            });
        }
        if key.len() < 20 {
            return Ok(KeyValidationResult {
                valid: false,
                error: Some("Clé trop courte".to_string()),
            });
        }
    }

    let validation = match provider.as_str() {
        "anthropic" => validate_anthropic(&key).await,
        "openrouter" => validate_openrouter(&key).await,
        _ => unreachable!(),
    };

    match validation {
        Ok(()) => {
            // Persist to keychain (best-effort — cache is authoritative this session)
            let _ = crate::ai_keys::save_key(&provider, &key);
            // Always cache in memory so generate_content works immediately
            state
                .key_cache
                .lock()
                .map_err(|e| e.to_string())?
                .insert(provider.clone(), key);
            Ok(KeyValidationResult {
                valid: true,
                error: None,
            })
        }
        Err(e) => Ok(KeyValidationResult {
            valid: false,
            error: Some(e),
        }),
    }
}

/// Returns masked key status for a provider (last 4 chars only).
#[tauri::command]
pub fn get_ai_key_status(provider: String) -> AiKeyStatus {
    match crate::ai_keys::get_key(&provider) {
        Ok(key) if !key.is_empty() => {
            let chars: Vec<char> = key.chars().collect();
            let last4: String = chars[chars.len().saturating_sub(4)..].iter().collect();
            AiKeyStatus {
                configured: true,
                masked: Some(format!("••••••••{last4}")),
            }
        }
        _ => AiKeyStatus {
            configured: false,
            masked: None,
        },
    }
}

#[tauri::command]
pub async fn delete_ai_key(
    state: tauri::State<'_, AppState>,
    provider: String,
) -> Result<(), String> {
    // Remove from cache
    let _ = state.key_cache.lock().map(|mut c| c.remove(&provider));
    // Remove from keychain (best-effort)
    let _ = crate::ai_keys::delete_key(&provider);
    Ok(())
}

#[tauri::command]
pub async fn test_ai_key(provider: String) -> Result<KeyValidationResult, String> {
    let key = crate::ai_keys::get_key(&provider)
        .map_err(|_| "Aucune clé configurée pour ce provider".to_string())?;
    let result = match provider.as_str() {
        "anthropic" => validate_anthropic(&key).await,
        "openrouter" => validate_openrouter(&key).await,
        _ => return Err(format!("Provider inconnu : {provider}")),
    };
    match result {
        Ok(()) => Ok(KeyValidationResult {
            valid: true,
            error: None,
        }),
        Err(e) => Ok(KeyValidationResult {
            valid: false,
            error: Some(e),
        }),
    }
}

#[tauri::command]
pub async fn set_active_provider(
    state: tauri::State<'_, AppState>,
    provider: String,
    model: String,
) -> Result<(), String> {
    // Persist to SQLite
    crate::db::settings_db::set(&state.db, "active_provider", &provider).await?;
    crate::db::settings_db::set(&state.db, "active_model", &model).await?;
    // Update in-memory cache
    let mut active = state.active_provider.lock().map_err(|e| e.to_string())?;
    active.provider = provider;
    active.model = model;
    Ok(())
}

#[tauri::command]
pub fn get_active_provider(state: tauri::State<'_, AppState>) -> Result<ProviderInfo, String> {
    let active = state.active_provider.lock().map_err(|e| e.to_string())?;
    Ok(ProviderInfo {
        provider: active.provider.clone(),
        model: active.model.clone(),
    })
}
