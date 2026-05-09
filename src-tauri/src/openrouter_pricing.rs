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
use std::sync::{Arc, OnceLock, RwLock};

const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

/// Shared HTTP client. `reqwest::Client` holds a connection pool internally;
/// rebuilding one per refresh defeats keep-alive and pays the TLS handshake
/// each time. Sourcery flagged this on PR #49 — kept as a `OnceLock` static
/// so the fire-and-forget startup task and manual user refreshes share one
/// pool. `OnceLock` is in std (no extra dep) and matches the convention used
/// by `sidecar::python_executable`.
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest::Client default config must build")
    })
}

/// One row of the cache. Fields match the shape `price_for` expects.
#[derive(Debug, Clone, Copy)]
pub struct LivePrice {
    /// USD per 1M input tokens.
    pub input_per_million: f64,
    /// USD per 1M output tokens.
    pub output_per_million: f64,
}

/// Snapshot of the live pricing table.
///
/// **Clone cost**: `PricingSnapshot` owns its `HashMap<String, (f64, f64)>`
/// directly, so `.clone()` performs a deep copy proportional to the number
/// of indexed models. At ~300-500 OpenRouter entries × ~50 bytes each the
/// per-clone cost is ~25 KB — fine for the few-times-per-session cadence
/// the Tauri commands clone at, but not "free" if a future polling
/// scheduler ever clones per-tick. If clone-cost ever matters, switch
/// `prices` to `Arc<HashMap<...>>` for cheap sharing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PricingSnapshot {
    /// `model_id → (input, output)` per million tokens, USD.
    /// Empty when no successful fetch has run yet.
    pub prices: HashMap<String, (f64, f64)>,
    /// When the last fetch completed. `None` until first success.
    /// Serialised as RFC 3339 so the JSON shape returned to the renderer
    /// stays compatible with `Date(string)` parsing in TypeScript.
    pub last_refreshed_at: Option<DateTime<Utc>>,
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

/// Parse an OpenRouter price string into `f64`. Returns `None` for missing
/// or unparseable values, `Some(0.0)` for the literal `"0"` (genuinely free
/// models). Pulled out so the refresh loop reads as a linear pipeline.
fn parse_price(raw: Option<&str>) -> Option<f64> {
    raw?.parse::<f64>().ok()
}

/// Fetch the full catalog and replace the cache. Returns the count of rows
/// indexed on success. Errors are stored in the snapshot's `last_error` and
/// returned to the caller so a manual-refresh UI can surface them.
pub async fn refresh(cache: &PricingCache) -> Result<usize, String> {
    let response = match http_client().get(OPENROUTER_MODELS_URL).send().await {
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

    // Convert per-token prices to per-million-tokens. Indexing rules,
    // post-Sourcery review:
    //
    // - Skip entries that don't carry a `pricing` block at all (incomplete
    //   catalog rows).
    // - Skip entries where BOTH prompt and completion fail to parse — that
    //   indicates a malformed row, not a free model. Genuine free models
    //   carry the literal string "0" (not "" or null); those parse cleanly
    //   to 0.0 and are KEPT, so the user sees them in the model picker
    //   alongside paid options.
    let mut prices = HashMap::with_capacity(body.data.len());
    for entry in body.data {
        let Some(p) = entry.pricing else { continue };
        let prompt = parse_price(p.prompt.as_deref());
        let completion = parse_price(p.completion.as_deref());
        // Both unparseable → drop. At least one parsed → keep, treating the
        // missing side as 0.0 so a model that only quotes prompt-cost
        // (rare) doesn't get its row priced as parse-failure-zero.
        let (Some(prompt_per_token), completion_per_token) = (prompt, completion.unwrap_or(0.0))
        else {
            // First binding failed: prompt unparseable. Try the inverse — if
            // completion alone is valid, keep with prompt = 0.
            if let Some(completion_per_token) = completion {
                prices.insert(entry.id, (0.0, completion_per_token * 1_000_000.0));
            }
            continue;
        };
        prices.insert(
            entry.id,
            (
                prompt_per_token * 1_000_000.0,
                completion_per_token * 1_000_000.0,
            ),
        );
    }

    let count = prices.len();
    let now = Utc::now();
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
    let Some(last) = snap.last_refreshed_at else {
        return true;
    };
    (Utc::now() - last).num_seconds() > max_age_secs
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
        cache.write().unwrap().last_refreshed_at = Some(Utc::now());
        assert!(!is_stale(&cache, 3600));
    }

    #[test]
    fn is_stale_true_when_older_than_max_age() {
        let cache = new_cache();
        let two_hours_ago = Utc::now() - chrono::Duration::seconds(7200);
        cache.write().unwrap().last_refreshed_at = Some(two_hours_ago);
        assert!(is_stale(&cache, 3600));
    }

    #[test]
    fn parse_price_handles_zero_string_as_free() {
        // OpenRouter quotes free models as "0" — must not be confused with
        // a parse failure (which `unwrap_or(0.0)` would mask).
        assert_eq!(parse_price(Some("0")), Some(0.0));
        assert_eq!(parse_price(Some("0.000003")), Some(0.000003));
    }

    #[test]
    fn parse_price_returns_none_on_missing_or_garbage() {
        assert_eq!(parse_price(None), None);
        assert_eq!(parse_price(Some("")), None);
        assert_eq!(parse_price(Some("not a number")), None);
    }
}
