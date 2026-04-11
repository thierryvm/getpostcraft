use serde::Serialize;
use crate::state::AppState;

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
