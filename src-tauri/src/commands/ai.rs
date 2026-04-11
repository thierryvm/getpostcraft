use serde::Serialize;
use crate::{db::history::PostRecord, state::AppState};

#[derive(Debug, Serialize, Clone)]
pub struct CaptionVariant {
    pub tone: String,
    pub caption: String,
    pub hashtags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GeneratedContent {
    pub caption: String,
    pub hashtags: Vec<String>,
}

/// Generates content by calling the Python sidecar.
/// Reads active provider + model from AppState, API key from OS keychain.
/// SECURITY: key is never logged or returned to renderer.
#[tauri::command]
pub async fn generate_content(
    state: tauri::State<'_, AppState>,
    brief: String,
    network: String,
) -> Result<GeneratedContent, String> {
    if brief.trim().len() < 10 {
        return Err("Le brief doit contenir au moins 10 caractères.".to_string());
    }

    // Snapshot provider info without holding the lock across await
    let (provider, model) = {
        let active = state.active_provider.lock().map_err(|e| e.to_string())?;
        (active.provider.clone(), active.model.clone())
    };

    // Ollama needs no key; others check cache first, then keychain
    let api_key = if provider == "ollama" {
        None
    } else {
        let cached = state
            .key_cache
            .lock()
            .ok()
            .and_then(|c| c.get(&provider).cloned());

        let key = cached
            .or_else(|| crate::ai_keys::get_key(&provider).ok())
            .ok_or_else(|| format!(
                "Aucune clé API pour « {provider} ». \
                 Configure-la dans Paramètres → Intelligence Artificielle."
            ))?;
        Some(key)
    };

    let base_url = match provider.as_str() {
        "ollama" => Some("http://localhost:11434/v1".to_string()),
        _ => None, // sidecar uses defaults
    };

    let request = crate::sidecar::SidecarRequest {
        action: "generate_content".to_string(),
        provider,
        api_key,
        model,
        base_url,
        brief,
        network: network.clone(),
        system_prompt: crate::network_rules::get_system_prompt(&network).to_string(),
    };

    let data = crate::sidecar::call_sidecar(request).await?;

    Ok(GeneratedContent {
        caption: data.caption,
        hashtags: data.hashtags,
    })
}

/// Generate 3 caption variants in parallel (educational / casual / punchy).
#[tauri::command]
pub async fn generate_variants(
    state: tauri::State<'_, AppState>,
    brief: String,
    network: String,
) -> Result<Vec<CaptionVariant>, String> {
    if brief.trim().len() < 10 {
        return Err("Le brief doit contenir au moins 10 caractères.".to_string());
    }

    let (provider, model) = {
        let active = state.active_provider.lock().map_err(|e| e.to_string())?;
        (active.provider.clone(), active.model.clone())
    };

    let api_key: Option<String> = if provider == "ollama" {
        None
    } else {
        let cached = state
            .key_cache
            .lock()
            .ok()
            .and_then(|c| c.get(&provider).cloned());
        let key = cached
            .or_else(|| crate::ai_keys::get_key(&provider).ok())
            .ok_or_else(|| {
                format!(
                    "Aucune clé API pour « {provider} ». \
                     Configure-la dans Paramètres → Intelligence Artificielle."
                )
            })?;
        Some(key)
    };

    let base_url: Option<String> = match provider.as_str() {
        "ollama" => Some("http://localhost:11434/v1".to_string()),
        _ => None,
    };

    let make_req = |tone: &str| crate::sidecar::SidecarRequest {
        action: "generate_content".to_string(),
        provider: provider.clone(),
        api_key: api_key.clone(),
        model: model.clone(),
        base_url: base_url.clone(),
        brief: brief.clone(),
        network: network.clone(),
        system_prompt: crate::network_rules::get_variant_prompt(&network, tone),
    };

    // Run all 3 in parallel — each spawns its own Python process
    let h_edu = tokio::task::spawn(crate::sidecar::call_sidecar(make_req("educational")));
    let h_cas = tokio::task::spawn(crate::sidecar::call_sidecar(make_req("casual")));
    let h_pun = tokio::task::spawn(crate::sidecar::call_sidecar(make_req("punchy")));

    let raw = [
        ("educational", h_edu.await.map_err(|e| e.to_string())?),
        ("casual",      h_cas.await.map_err(|e| e.to_string())?),
        ("punchy",      h_pun.await.map_err(|e| e.to_string())?),
    ];
    let mut variants = Vec::with_capacity(3);
    for (tone, res) in raw {
        match res {
            Ok(data) => variants.push(CaptionVariant {
                tone: tone.to_string(),
                caption: data.caption,
                hashtags: data.hashtags,
            }),
            Err(e) => return Err(format!("Erreur variante {tone}: {e}")),
        }
    }
    Ok(variants)
}

/// Fire-and-forget sidecar warmup — called when Composer mounts.
/// Validates Python + module availability so the first generation is faster.
/// Never returns an error to the UI (failures are silent / logged to stderr).
#[tauri::command]
pub async fn warmup_sidecar() -> () {
    use serde::Serialize;

    #[derive(Serialize)]
    struct WarmupRequest {
        action: &'static str,
    }

    let req = WarmupRequest { action: "warmup" };
    if let Ok(json) = serde_json::to_string(&req) {
        let _ = crate::sidecar::run_sidecar_raw(json, 15).await;
    }
}

/// Save a generated post as draft in SQLite history.
#[tauri::command]
pub async fn save_draft(
    state: tauri::State<'_, AppState>,
    network: String,
    caption: String,
    hashtags: Vec<String>,
) -> Result<i64, String> {
    crate::db::history::insert_draft(&state.db, &network, &caption, &hashtags).await
}

/// Fetch recent post history from SQLite.
#[tauri::command]
pub async fn get_post_history(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<PostRecord>, String> {
    crate::db::history::list_recent(&state.db, limit.unwrap_or(50)).await
}
