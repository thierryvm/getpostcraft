use crate::{db::history::PostRecord, state::AppState};
use serde::{Deserialize, Serialize};

/// A single slide in a carousel (index is 1-based).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CarouselSlide {
    pub index: u8,
    pub total: u8,
    pub emoji: String,
    pub title: String,
    pub body: String,
}

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
            .ok_or_else(|| {
                format!(
                    "Aucune clé API pour « {provider} ». \
                 Configure-la dans Paramètres → Intelligence Artificielle."
                )
            })?;
        Some(key)
    };

    let base_url = match provider.as_str() {
        "ollama" => Some("http://localhost:11434/v1".to_string()),
        _ => None, // sidecar uses defaults
    };

    let request = crate::sidecar::SidecarRequest {
        action: "generate_content".to_string(),
        provider: provider.clone(),
        api_key,
        model: model.clone(),
        base_url,
        brief,
        network: network.clone(),
        system_prompt: crate::network_rules::get_system_prompt(&network).to_string(),
    };

    log::info!("AI: generating content — provider={provider} model={model} network={network}");

    let data = crate::sidecar::call_sidecar(request).await.map_err(|e| {
        log::error!("AI: generation failed — provider={provider} model={model}: {e}");
        e
    })?;

    log::info!(
        "AI: generation success — caption_len={} hashtags={}",
        data.caption.len(),
        data.hashtags.len()
    );

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
        ("casual", h_cas.await.map_err(|e| e.to_string())?),
        ("punchy", h_pun.await.map_err(|e| e.to_string())?),
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

/// Scrape a URL and return extracted text suitable for use as a brief.
#[tauri::command]
pub async fn scrape_url_for_brief(url: String) -> Result<String, String> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct ScrapeRequest {
        action: &'static str,
        url: String,
        max_chars: u32,
    }

    let req = ScrapeRequest {
        action: "scrape_url",
        url,
        max_chars: 3000,
    };
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    let stdout = crate::sidecar::run_sidecar_raw(json, 20).await?;

    #[derive(serde::Deserialize)]
    struct ScrapeData {
        text: String,
    }
    #[derive(serde::Deserialize)]
    struct ScrapeResp {
        ok: bool,
        data: Option<ScrapeData>,
        error: Option<String>,
    }

    let resp: ScrapeResp =
        serde_json::from_str(&stdout).map_err(|e| format!("Parse scrape response: {e}"))?;

    if resp.ok {
        resp.data
            .map(|d| d.text)
            .ok_or_else(|| "No text returned".to_string())
    } else {
        Err(resp.error.unwrap_or_else(|| "Scrape failed".to_string()))
    }
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

/// Generate structured carousel slides (AI → Vec<CarouselSlide>).
#[tauri::command]
pub async fn generate_carousel(
    state: tauri::State<'_, AppState>,
    brief: String,
    network: String,
    slide_count: u8,
) -> Result<Vec<CarouselSlide>, String> {
    if brief.trim().len() < 10 {
        return Err("Le brief doit contenir au moins 10 caractères.".to_string());
    }
    let slide_count = slide_count.clamp(3, 10);

    let (provider, model) = {
        let active = state.active_provider.lock().map_err(|e| e.to_string())?;
        (active.provider.clone(), active.model.clone())
    };

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
            .ok_or_else(|| {
                format!(
                    "Aucune clé API pour « {provider} ». \
                 Configure-la dans Paramètres → Intelligence Artificielle."
                )
            })?;
        Some(key)
    };

    let base_url = match provider.as_str() {
        "ollama" => Some("http://localhost:11434/v1".to_string()),
        _ => None,
    };

    #[derive(Serialize)]
    struct CarouselRequest {
        action: &'static str,
        provider: String,
        api_key: Option<String>,
        model: String,
        base_url: Option<String>,
        brief: String,
        network: String,
        slide_count: u8,
        system_prompt: String,
    }

    let req = CarouselRequest {
        action: "generate_carousel",
        provider,
        api_key,
        model,
        base_url,
        brief,
        network: network.clone(),
        slide_count,
        system_prompt: crate::network_rules::get_carousel_prompt(&network, slide_count),
    };
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    let stdout = crate::sidecar::run_sidecar_raw(json, 45).await?;

    #[derive(Deserialize)]
    struct SlideData {
        emoji: String,
        title: String,
        body: String,
    }
    #[derive(Deserialize)]
    struct CarouselData {
        slides: Vec<SlideData>,
    }
    #[derive(Deserialize)]
    struct CarouselResp {
        ok: bool,
        data: Option<CarouselData>,
        error: Option<String>,
    }

    let resp: CarouselResp =
        serde_json::from_str(&stdout).map_err(|e| format!("Parse carousel response: {e}"))?;

    if !resp.ok {
        return Err(resp
            .error
            .unwrap_or_else(|| "Carousel generation failed".to_string()));
    }

    let slides_data = resp
        .data
        .ok_or_else(|| "No carousel data returned".to_string())?
        .slides;
    let total = slides_data.len() as u8;

    Ok(slides_data
        .into_iter()
        .enumerate()
        .map(|(i, s)| CarouselSlide {
            index: i as u8 + 1,
            total,
            emoji: s.emoji,
            title: s.title,
            body: s.body,
        })
        .collect())
}

/// Save a generated post as draft in SQLite history.
#[tauri::command]
pub async fn save_draft(
    state: tauri::State<'_, AppState>,
    network: String,
    caption: String,
    hashtags: Vec<String>,
    image_path: Option<String>,
) -> Result<i64, String> {
    crate::db::history::insert_draft(
        &state.db,
        &network,
        &caption,
        &hashtags,
        image_path.as_deref(),
    )
    .await
}

/// Fetch recent post history from SQLite.
#[tauri::command]
pub async fn get_post_history(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<PostRecord>, String> {
    crate::db::history::list_recent(&state.db, limit.unwrap_or(50)).await
}
