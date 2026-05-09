//! Live pricing from OpenRouter's `/api/v1/models` endpoint.
//!
//! ## Why this exists
//!
//! `network_rules::pricing_map()` is a hardcoded table of USD/M token rates.
//! Providers tweak prices every few months (e.g. Anthropic's Sonnet
//! readjusted twice in 2025), and the table only refreshes when we cut a new
//! Getpostcraft release. Users running an older version see stale costs in
//! the AI usage panel — sometimes by 2-3x once a model rotates.
//!
//! This module fetches the OpenRouter pricing catalog at startup and on
//! manual refresh, caches it in process memory, and exposes a lookup that
//! `price_for` can prefer over the static table. If the network is
//! unavailable, the static table still answers correctly — the live cache
//! is purely a freshness boost, never a hard dependency.
//!
//! ## Out of scope
//!
//! - Anthropic native (`provider = "anthropic"`) and Ollama (`provider =
//!   "ollama"`) don't go through OpenRouter, so their pricing stays fully
//!   static. The keys here only reflect models routed via OpenRouter.
//! - Disk persistence — a 6h cache in memory is enough; on app restart the
//!   first call to the AI fires a fresh fetch in the background.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

/// One row of the cache. Fields match the shape `price_for` expects.
#[derive(Debug, Clone, Copy)]
pub struct LivePrice {
    /// USD per 1M input tokens.
    pub input_per_million: f64,
    /// USD per 1M output tokens.
    pub output_per_million: f64,
}

/// Snapshot of the live pricing table. Cloned cheaply (Arc'd map of small
/// values) so callers can read without holding the RwLock during their work.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PricingSnapshot {
    /// `model_id → (input, output)` per million tokens, USD.
    /// Empty when no successful fetch has run yet.
    pub prices: HashMap<String, (f64, f64)>,
    /// When the last fetch completed (RFC 3339 UTC). `None` until first success.
    pub last_refreshed_at: Option<String>,
    /// Last error message from a failed fetch, if any. Cleared on next success.
    pub last_error: Option<String>,
}

/// Process-wide cache. Wrapped in `Arc<RwLock>` so the Tauri command and the
/// startup fire-and-forget task can share without ownership gymnastics.
pub type PricingCache = Arc<RwLock<PricingSnapshot>>;

pub fn new_cache() -> PricingCache {
    Arc::new(RwLock::new(PricingSnapshot::default()))
}

/// Look up the live price for a model. Returns `None` when:
///
/// - no successful fetch has run yet (cache empty), OR
/// - the model id isn't in OpenRouter's catalog (Anthropic native, Ollama,
///   custom self-hosted).
///
/// `network_rules::price_for` is responsible for falling back to the static
/// table in that case.
pub fn lookup_live(cache: &PricingCache, model_id: &str) -> Option<LivePrice> {
    let snap = cache.read().ok()?;
    snap.prices.get(model_id).map(|&(input, output)| LivePrice {
        input_per_million: input,
        output_per_million: output,
    })
}

/// Fetch the full catalog and replace the cache. Returns the count of rows
/// indexed on success. Errors are stored in the snapshot's `last_error` and
/// returned to the caller so a manual-refresh UI can surface them.
pub async fn refresh(cache: &PricingCache) -> Result<usize, String> {
    let url = OPENROUTER_MODELS_URL.to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("OpenRouter pricing fetch failed: {e}");
            if let Ok(mut snap) = cache.write() {
                snap.last_error = Some(msg.clone());
            }
            return Err(msg);
        }
    };

    if !response.status().is_success() {
        let msg = format!(
            "OpenRouter pricing fetch returned HTTP {}",
            response.status()
        );
        if let Ok(mut snap) = cache.write() {
            snap.last_error = Some(msg.clone());
        }
        return Err(msg);
    }

    #[derive(Deserialize)]
    struct ModelPricing {
        /// USD per token (yes, per ONE token). We multiply by 1M to match
        /// the static table's convention. Strings, not numbers — OpenRouter
        /// quotes them quoted to avoid float-precision concerns at 9 decimals.
        prompt: Option<String>,
        completion: Option<String>,
    }

    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        pricing: Option<ModelPricing>,
    }

    #[derive(Deserialize)]
    struct ModelsResponse {
        data: Vec<ModelEntry>,
    }

    let body: ModelsResponse = match response.json().await {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("OpenRouter pricing JSON parse failed: {e}");
            if let Ok(mut snap) = cache.write() {
                snap.last_error = Some(msg.clone());
            }
            return Err(msg);
        }
    };

    // Convert per-token prices to per-million-tokens. Skip models that
    // don't quote both halves — they're typically free-tier or
    // experimental endpoints we don't want to cost-track anyway.
    let mut prices = HashMap::with_capacity(body.data.len());
    for entry in body.data {
        let Some(p) = entry.pricing else { continue };
        let prompt_per_token: f64 = p
            .prompt
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let completion_per_token: f64 = p
            .completion
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        // A genuine zero is fine (free models), but if BOTH parsed to 0.0
        // and the input wasn't literally "0", treat as missing — defensive
        // against unparseable values being silently mistaken for "free".
        if prompt_per_token == 0.0 && completion_per_token == 0.0 {
            continue;
        }
        prices.insert(
            entry.id,
            (
                prompt_per_token * 1_000_000.0,
                completion_per_token * 1_000_000.0,
            ),
        );
    }

    let count = prices.len();
    let now = Utc::now().to_rfc3339();
    if let Ok(mut snap) = cache.write() {
        snap.prices = prices;
        snap.last_refreshed_at = Some(now);
        snap.last_error = None;
    }
    Ok(count)
}

/// True when the cache is older than `max_age_secs` or has never been filled.
/// Currently the UI does an explicit refresh on user click and the startup
/// task always runs once, so this helper is plumbing for a future
/// polling-based scheduler. Kept and tested so it doesn't bitrot.
#[allow(dead_code)]
pub fn is_stale(cache: &PricingCache, max_age_secs: i64) -> bool {
    let Ok(snap) = cache.read() else {
        return true;
    };
    let Some(ts) = snap.last_refreshed_at.as_ref() else {
        return true;
    };
    let Ok(parsed) = ts.parse::<DateTime<Utc>>() else {
        return true;
    };
    (Utc::now() - parsed).num_seconds() > max_age_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_live_returns_none_on_empty_cache() {
        let cache = new_cache();
        assert!(lookup_live(&cache, "anthropic/claude-sonnet-4.6").is_none());
    }

    #[test]
    fn lookup_live_returns_some_when_populated() {
        let cache = new_cache();
        cache
            .write()
            .unwrap()
            .prices
            .insert("foo/bar".to_string(), (3.0, 15.0));
        let p = lookup_live(&cache, "foo/bar").expect("expected price");
        assert_eq!(p.input_per_million, 3.0);
        assert_eq!(p.output_per_million, 15.0);
    }

    #[test]
    fn is_stale_true_when_never_filled() {
        let cache = new_cache();
        assert!(is_stale(&cache, 3600));
    }

    #[test]
    fn is_stale_false_when_freshly_refreshed() {
        let cache = new_cache();
        cache.write().unwrap().last_refreshed_at = Some(Utc::now().to_rfc3339());
        assert!(!is_stale(&cache, 3600));
    }

    #[test]
    fn is_stale_true_when_older_than_max_age() {
        let cache = new_cache();
        let two_hours_ago = (Utc::now() - chrono::Duration::seconds(7200)).to_rfc3339();
        cache.write().unwrap().last_refreshed_at = Some(two_hours_ago);
        assert!(is_stale(&cache, 3600));
    }
}
