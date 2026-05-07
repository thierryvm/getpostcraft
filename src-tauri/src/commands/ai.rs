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
/// If account_id is provided, fetches the account's product_truth and injects it into the prompt.
/// SECURITY: key is never logged or returned to renderer.
#[tauri::command]
pub async fn generate_content(
    state: tauri::State<'_, AppState>,
    brief: String,
    network: String,
    account_id: Option<i64>,
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

    // Fetch product_truth from the selected account (if any)
    let product_truth = if let Some(aid) = account_id {
        crate::db::accounts::get_by_id(&state.db, aid)
            .await
            .ok()
            .and_then(|a| a.product_truth)
    } else {
        None
    };

    let base_prompt = crate::network_rules::get_system_prompt(&network);
    let system_prompt =
        crate::network_rules::inject_product_truth(base_prompt, product_truth.as_deref());

    let request = crate::sidecar::SidecarRequest {
        action: "generate_content".to_string(),
        provider: provider.clone(),
        api_key,
        model: model.clone(),
        base_url,
        brief,
        network: network.clone(),
        system_prompt,
    };

    log::info!(
        "AI: generating content — provider={provider} model={model} network={network} \
         account_id={account_id:?} product_truth={}",
        product_truth.is_some()
    );

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
    account_id: Option<i64>,
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

    let product_truth: Option<String> = if let Some(aid) = account_id {
        crate::db::accounts::get_by_id(&state.db, aid)
            .await
            .ok()
            .and_then(|a| a.product_truth)
    } else {
        None
    };

    let make_req = |tone: &str| crate::sidecar::SidecarRequest {
        action: "generate_content".to_string(),
        provider: provider.clone(),
        api_key: api_key.clone(),
        model: model.clone(),
        base_url: base_url.clone(),
        brief: brief.clone(),
        network: network.clone(),
        system_prompt: crate::network_rules::get_variant_prompt_with_truth(
            &network,
            tone,
            product_truth.as_deref(),
        ),
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

/// Render a URL with Playwright and synthesize a ProductTruth block from the
/// page's visible text. Two-step: scrape → AI synthesis. Returns the plain-text
/// ProductTruth ready to paste into the account's textarea.
///
/// Uses the active provider/model. We pick `claude-sonnet-4.6` quality for the
/// synthesis even on Haiku-default accounts because the cost is one-shot per
/// onboarding (~$0.005) and the output frames every future post.
#[tauri::command]
pub async fn synthesize_product_truth_from_url(
    state: tauri::State<'_, AppState>,
    url: String,
    handle: String,
) -> Result<String, String> {
    use serde::Serialize;

    if url.trim().is_empty() {
        return Err("URL vide.".to_string());
    }

    // 1. Snapshot provider + key without holding the lock across await.
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
        Some(
            cached
                .or_else(|| crate::ai_keys::get_key(&provider).ok())
                .ok_or_else(|| {
                    format!(
                        "Aucune clé API pour « {provider} ». Configure-la dans \
                         Paramètres → Intelligence Artificielle."
                    )
                })?,
        )
    };
    let base_url = match provider.as_str() {
        "ollama" => Some("http://localhost:11434/v1".to_string()),
        _ => None,
    };

    // 2. Scrape the page via Playwright (handles SPAs).
    log::info!("synthesize_product_truth: rendering {url}");
    #[derive(Serialize)]
    struct ScrapeRequest<'a> {
        action: &'a str,
        url: &'a str,
        max_chars: u32,
    }
    let scrape_req = ScrapeRequest {
        action: "scrape_url_rendered",
        url: &url,
        max_chars: 8000,
    };
    let scrape_json = serde_json::to_string(&scrape_req).map_err(|e| e.to_string())?;
    // Playwright launch + networkidle wait can take ~10-15 s on slow connections.
    let scrape_stdout = crate::sidecar::run_sidecar_raw(scrape_json, 60).await?;

    #[derive(Deserialize)]
    struct ScrapeData {
        text: String,
    }
    #[derive(Deserialize)]
    struct ScrapeResp {
        ok: bool,
        data: Option<ScrapeData>,
        error: Option<String>,
    }
    let scrape_resp: ScrapeResp =
        serde_json::from_str(&scrape_stdout).map_err(|e| format!("Parse scrape response: {e}"))?;
    let content = if scrape_resp.ok {
        scrape_resp
            .data
            .map(|d| d.text)
            .ok_or_else(|| "Scraper a retourné une réponse vide".to_string())?
    } else {
        return Err(scrape_resp
            .error
            .unwrap_or_else(|| "Échec scraping site".to_string()));
    };

    if content.trim().len() < 50 {
        return Err(format!(
            "Le site n'a renvoyé que {} caractères — vérifie l'URL ou le SPA bloque le rendu.",
            content.trim().len()
        ));
    }

    // 3. Synthesize the ProductTruth via the AI provider.
    log::info!(
        "synthesize_product_truth: synthesizing with {provider}/{model} \
         from {} chars of content",
        content.len()
    );
    let system_prompt = crate::network_rules::get_synthesis_prompt(&handle);

    #[derive(Serialize)]
    struct SynthesisRequest {
        action: &'static str,
        provider: String,
        api_key: Option<String>,
        model: String,
        base_url: Option<String>,
        content: String,
        system_prompt: String,
    }
    let synth_req = SynthesisRequest {
        action: "synthesize_product_truth",
        provider: provider.clone(),
        api_key,
        model: model.clone(),
        base_url,
        content,
        system_prompt,
    };
    let synth_json = serde_json::to_string(&synth_req).map_err(|e| e.to_string())?;
    let synth_stdout = crate::sidecar::run_sidecar_raw(synth_json, 60).await?;

    #[derive(Deserialize)]
    struct SynthData {
        product_truth: String,
    }
    #[derive(Deserialize)]
    struct SynthResp {
        ok: bool,
        data: Option<SynthData>,
        error: Option<String>,
    }
    let synth_resp: SynthResp = serde_json::from_str(&synth_stdout)
        .map_err(|e| format!("Parse synthesis response: {e}"))?;

    if synth_resp.ok {
        let truth = synth_resp
            .data
            .map(|d| d.product_truth)
            .ok_or_else(|| "Synthèse vide".to_string())?;
        log::info!(
            "synthesize_product_truth: success — {} chars produced",
            truth.len()
        );
        Ok(truth)
    } else {
        Err(synth_resp
            .error
            .unwrap_or_else(|| "Échec synthèse IA".to_string()))
    }
}

/// Result of `analyze_url_visual` — pairs the textual ProductTruth synthesis
/// with the structured visual profile so the UI can render both in one pass.
#[derive(Debug, Serialize)]
pub struct WebsiteAnalysis {
    pub product_truth: String,
    pub visual_profile: serde_json::Value,
}

/// Render a URL with Playwright (single browser launch), extract both the
/// rendered text AND a viewport screenshot, then run two AI passes:
/// - Sonnet 4.6 on the text → structured ProductTruth (textual)
/// - Vision (same model) on the screenshot → JSON visual profile
///
/// Persists the visual_profile on the account so future post generations can
/// reuse it (color tokens, typography hints) without re-paying Vision per post.
#[tauri::command]
pub async fn analyze_url_visual(
    state: tauri::State<'_, AppState>,
    url: String,
    handle: String,
    account_id: Option<i64>,
) -> Result<WebsiteAnalysis, String> {
    if url.trim().is_empty() {
        return Err("URL vide.".to_string());
    }

    // Snapshot provider + key (single resolution shared by both AI calls).
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
        Some(
            cached
                .or_else(|| crate::ai_keys::get_key(&provider).ok())
                .ok_or_else(|| {
                    format!(
                        "Aucune clé API pour « {provider} ». Configure-la dans \
                         Paramètres → Intelligence Artificielle."
                    )
                })?,
        )
    };
    let base_url = match provider.as_str() {
        "ollama" => Some("http://localhost:11434/v1".to_string()),
        _ => None,
    };

    // Step 1 — single Playwright launch for text + screenshot.
    log::info!("analyze_url_visual: rendering {url} with screenshot");
    #[derive(Serialize)]
    struct ScrapeRequest<'a> {
        action: &'a str,
        url: &'a str,
        max_chars: u32,
        capture_screenshot: bool,
    }
    let scrape_req = ScrapeRequest {
        action: "scrape_url_rendered_with_screenshot",
        url: &url,
        max_chars: 8000,
        capture_screenshot: true,
    };
    let scrape_json = serde_json::to_string(&scrape_req).map_err(|e| e.to_string())?;
    let scrape_stdout = crate::sidecar::run_sidecar_raw(scrape_json, 60).await?;

    #[derive(Deserialize)]
    struct ScrapeData {
        text: String,
        screenshot: Option<String>,
    }
    #[derive(Deserialize)]
    struct ScrapeResp {
        ok: bool,
        data: Option<ScrapeData>,
        error: Option<String>,
    }
    let scrape_resp: ScrapeResp =
        serde_json::from_str(&scrape_stdout).map_err(|e| format!("Parse scrape response: {e}"))?;
    let scrape_data = if scrape_resp.ok {
        scrape_resp
            .data
            .ok_or_else(|| "Scraper a retourné une réponse vide".to_string())?
    } else {
        return Err(scrape_resp
            .error
            .unwrap_or_else(|| "Échec scraping site".to_string()));
    };

    if scrape_data.text.trim().len() < 50 {
        return Err(format!(
            "Le site n'a renvoyé que {} caractères — vérifie l'URL ou le SPA bloque le rendu.",
            scrape_data.text.trim().len()
        ));
    }
    let screenshot = scrape_data
        .screenshot
        .ok_or_else(|| "Le scraper n'a pas renvoyé de screenshot".to_string())?;

    // Step 2 — synthesize the textual ProductTruth.
    log::info!(
        "analyze_url_visual: synthesizing product truth ({} chars input)",
        scrape_data.text.len()
    );
    let synthesis_system_prompt = crate::network_rules::get_synthesis_prompt(&handle);

    #[derive(Serialize)]
    struct SynthesisRequest {
        action: &'static str,
        provider: String,
        api_key: Option<String>,
        model: String,
        base_url: Option<String>,
        content: String,
        system_prompt: String,
    }
    let synth_req = SynthesisRequest {
        action: "synthesize_product_truth",
        provider: provider.clone(),
        api_key: api_key.clone(),
        model: model.clone(),
        base_url: base_url.clone(),
        content: scrape_data.text,
        system_prompt: synthesis_system_prompt,
    };
    let synth_json = serde_json::to_string(&synth_req).map_err(|e| e.to_string())?;
    let synth_stdout = crate::sidecar::run_sidecar_raw(synth_json, 60).await?;

    #[derive(Deserialize)]
    struct SynthData {
        product_truth: String,
    }
    #[derive(Deserialize)]
    struct SynthResp {
        ok: bool,
        data: Option<SynthData>,
        error: Option<String>,
    }
    let synth_resp: SynthResp = serde_json::from_str(&synth_stdout)
        .map_err(|e| format!("Parse synthesis response: {e}"))?;
    let product_truth = if synth_resp.ok {
        synth_resp
            .data
            .map(|d| d.product_truth)
            .ok_or_else(|| "Synthèse vide".to_string())?
    } else {
        return Err(synth_resp
            .error
            .unwrap_or_else(|| "Échec synthèse IA".to_string()));
    };

    // Step 3 — Vision pass on the screenshot.
    log::info!("analyze_url_visual: extracting visual profile via Vision");
    #[derive(Serialize)]
    struct VisualRequest {
        action: &'static str,
        provider: String,
        api_key: Option<String>,
        model: String,
        base_url: Option<String>,
        screenshot: String,
        system_prompt: String,
    }
    let visual_req = VisualRequest {
        action: "extract_visual_profile",
        provider,
        api_key,
        model,
        base_url,
        screenshot,
        system_prompt: crate::network_rules::get_visual_extraction_prompt().to_string(),
    };
    let visual_json = serde_json::to_string(&visual_req).map_err(|e| e.to_string())?;
    let visual_stdout = crate::sidecar::run_sidecar_raw(visual_json, 60).await?;

    #[derive(Deserialize)]
    struct VisualResp {
        ok: bool,
        data: Option<serde_json::Value>,
        error: Option<String>,
    }
    let visual_resp: VisualResp =
        serde_json::from_str(&visual_stdout).map_err(|e| format!("Parse visual response: {e}"))?;
    let visual_profile = if visual_resp.ok {
        visual_resp
            .data
            .ok_or_else(|| "Visual profile vide".to_string())?
    } else {
        return Err(visual_resp
            .error
            .unwrap_or_else(|| "Échec extraction visuelle".to_string()));
    };

    // Step 4 — persist the visual profile on the account if one was provided.
    // Failure to persist is non-fatal — the UI still gets the result and the
    // user can manually retry. Errors are logged for diagnosis.
    if let Some(aid) = account_id {
        let visual_str = serde_json::to_string(&visual_profile).map_err(|e| e.to_string())?;
        if let Err(e) =
            crate::db::accounts::update_visual_profile(&state.db, aid, Some(&visual_str)).await
        {
            log::warn!("analyze_url_visual: persist failed for account {aid}: {e}");
        }
    }

    Ok(WebsiteAnalysis {
        product_truth,
        visual_profile,
    })
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
    account_id: Option<i64>,
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

    let product_truth: Option<String> = if let Some(aid) = account_id {
        crate::db::accounts::get_by_id(&state.db, aid)
            .await
            .ok()
            .and_then(|a| a.product_truth)
    } else {
        None
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
        system_prompt: {
            let base = crate::network_rules::get_carousel_prompt(&network, slide_count);
            crate::network_rules::inject_product_truth(&base, product_truth.as_deref())
        },
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

/// Fetch a single post by id — used by the composer when reloading a draft
/// or a published post selected from the calendar/history view.
#[tauri::command]
pub async fn get_post_by_id(
    state: tauri::State<'_, AppState>,
    post_id: i64,
) -> Result<PostRecord, String> {
    crate::db::history::get_by_id(&state.db, post_id).await
}
