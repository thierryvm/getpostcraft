/// Sanitize upstream API error bodies before they reach the logger.
///
/// ## Why this exists
///
/// During OAuth flows, the publisher logs response bodies from Instagram
/// (Meta Graph) and LinkedIn when token exchange fails. The documented
/// error responses don't echo the access_token — but defense-in-depth
/// against API behavior changes (or a misconfigured proxy that mirrors
/// the request) is cheap insurance.
///
/// PR-X moved tokens to the OS keyring; PR-S2 closes the last seam where
/// a token could land in a log file.
///
/// ## What it does
///
/// 1. Truncates input to 500 chars (char-boundary safe) so an HTML error
///    page can't flood the logs.
/// 2. Replaces known secret-bearing fields with `[REDACTED]`:
///    - JSON-style: `"access_token": "..."` → `"access_token": "[REDACTED]"`
///    - URL-encoded: `access_token=...` → `access_token=[REDACTED]`
/// 3. Field name match is case-insensitive (Meta/LinkedIn are consistent
///    with snake_case but a future provider might not be).
///
/// ## What it deliberately does NOT do
///
/// - High-entropy heuristic redaction (anything alphanumeric > 32 chars).
///   Too many false positives on URLs, IDs, hashes — would gut the debug
///   value of the log without meaningfully improving safety.
/// - JSON parsing. The bodies we care about may be partial, malformed,
///   or HTML — a string-level redact survives all three.
use regex::Regex;
use std::sync::OnceLock;

/// Cap on logged body length. Short enough to keep logs readable and
/// long enough to debug provider error messages (Meta error.message is
/// usually < 200 chars).
const MAX_LEN: usize = 500;

static JSON_FIELD_RE: OnceLock<Regex> = OnceLock::new();
static URL_FIELD_RE: OnceLock<Regex> = OnceLock::new();
static AI_KEY_RE: OnceLock<Regex> = OnceLock::new();

fn json_field_re() -> &'static Regex {
    JSON_FIELD_RE.get_or_init(|| {
        // Matches `"field": "value"` with optional whitespace around the colon.
        // The value group accepts anything except an unescaped quote so we
        // cover escaped quotes inside the value (rare but possible).
        // Capture 1: field name (preserved in replacement to keep original case).
        Regex::new(
            r#"(?i)"(access_token|refresh_token|client_secret|password|authorization|bearer|api_key|short_lived_token|long_lived_token)"\s*:\s*"(?:[^"\\]|\\.)*""#,
        )
        .expect("JSON redaction regex must be valid")
    })
}

fn url_field_re() -> &'static Regex {
    URL_FIELD_RE.get_or_init(|| {
        // Matches `field=value` until next `&` or whitespace. Keeps the
        // field name (capture 1) so the surrounding context stays readable.
        Regex::new(r"(?i)\b(access_token|refresh_token|client_secret|password|api_key)=[^&\s]+")
            .expect("URL redaction regex must be valid")
    })
}

fn ai_key_re() -> &'static Regex {
    AI_KEY_RE.get_or_init(|| {
        // Catches AI provider API keys regardless of context (named JSON
        // field, raw error body, stack trace). Defense-in-depth beyond the
        // field-matching scrubbers above — covers cases like OpenAI SDK
        // errors ("Incorrect API key provided: sk-or-v1-...") that
        // propagate via `log::error!` without the key sitting in a
        // recognised field name.
        //
        // Covered prefixes (>= 20 chars after the prefix to avoid matching
        // short test fixtures or unrelated `sk-` substrings):
        //   sk-or-v1-*  OpenRouter
        //   sk-ant-*    Anthropic native
        //   sk-proj-*   OpenAI project keys
        //   sk-*        OpenAI legacy + generic fallback (Mistral, DeepSeek, Groq)
        //   AIza*       Google Gemini
        Regex::new(r"\b(?:sk-(?:or-v1-|ant-|proj-)?[A-Za-z0-9_\-]{20,}|AIza[A-Za-z0-9_\-]{20,})\b")
            .expect("AI key redaction regex must be valid")
    })
}

/// Truncate at a UTF-8 char boundary so we never split a multi-byte char
/// (would panic in `&str` slicing). Walks back from `MAX_LEN` until valid.
fn truncate_safely(s: &str) -> &str {
    if s.len() <= MAX_LEN {
        return s;
    }
    let mut end = MAX_LEN;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn redact_secrets(input: &str) -> String {
    let truncated = truncate_safely(input);

    let after_json = json_field_re().replace_all(truncated, |caps: &regex::Captures| {
        format!(r#""{}": "[REDACTED]""#, &caps[1])
    });

    let after_url = url_field_re().replace_all(&after_json, |caps: &regex::Captures| {
        format!("{}=[REDACTED]", &caps[1])
    });

    // Final pass: catch AI provider keys that survived the field-based
    // redactors (e.g. an SDK error string echoing the key without naming
    // a field). Marked with a distinct token so the log reader knows the
    // upstream message included a raw key — useful for triage when a
    // misbehaving provider mirrors auth headers into errors.
    let after_ai = ai_key_re().replace_all(&after_url, "[REDACTED_AI_KEY]");

    let mut out = after_ai.into_owned();
    if input.len() > MAX_LEN {
        out.push_str("…[truncated]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_json_access_token() {
        let body = r#"{"access_token":"IGQVJSecret123","user_id":"42"}"#;
        let out = redact_secrets(body);
        assert!(!out.contains("IGQVJSecret123"));
        assert!(out.contains("[REDACTED]"));
        assert!(out.contains("user_id"), "non-secret fields preserved");
        assert!(out.contains("42"), "non-secret values preserved");
    }

    #[test]
    fn redacts_json_with_whitespace_around_colon() {
        let body = r#"{ "access_token" :   "IGQVJSecret123" }"#;
        let out = redact_secrets(body);
        assert!(!out.contains("IGQVJSecret123"));
    }

    #[test]
    fn redacts_url_encoded_access_token() {
        let body = "access_token=IGQVJSecret&error=invalid";
        let out = redact_secrets(body);
        assert!(!out.contains("IGQVJSecret"));
        assert!(out.contains("error=invalid"), "non-secret kept");
    }

    #[test]
    fn redacts_multiple_fields() {
        let body = r#"{"access_token":"AAA","refresh_token":"BBB","client_secret":"CCC"}"#;
        let out = redact_secrets(body);
        assert!(!out.contains("AAA"));
        assert!(!out.contains("BBB"));
        assert!(!out.contains("CCC"));
    }

    #[test]
    fn case_insensitive_field_match() {
        // Hypothetical provider using PascalCase fields — should still redact.
        let body = r#"{"Access_Token":"secret123"}"#;
        let out = redact_secrets(body);
        assert!(!out.contains("secret123"));
    }

    #[test]
    fn preserves_safe_error_message() {
        let body =
            r#"{"error":{"message":"Invalid OAuth code","type":"OAuthException","code":190}}"#;
        let out = redact_secrets(body);
        // No secrets present — output should be effectively unchanged.
        assert!(out.contains("Invalid OAuth code"));
        assert!(out.contains("OAuthException"));
        assert!(out.contains("190"));
    }

    #[test]
    fn truncates_oversized_body() {
        let body = "x".repeat(2000);
        let out = redact_secrets(&body);
        assert!(out.len() <= MAX_LEN + 32, "output capped near MAX_LEN");
        assert!(out.contains("[truncated]"));
    }

    #[test]
    fn truncate_does_not_split_multibyte_chars() {
        // A string of 3-byte chars sized to put MAX_LEN mid-character.
        let body = "é".repeat(MAX_LEN); // 'é' is 2 bytes — repeated MAX_LEN times = 2*MAX_LEN bytes.
        let out = redact_secrets(&body);
        // Must not panic on UTF-8 boundary; length capped.
        assert!(out.len() < body.len());
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(redact_secrets(""), "");
    }

    #[test]
    fn html_error_page_does_not_panic() {
        // Sometimes APIs return HTML — our redactor should just truncate it.
        let body = "<html><body>500 Internal Server Error</body></html>";
        let out = redact_secrets(body);
        assert!(out.contains("500"));
    }

    #[test]
    fn url_field_only_matches_exact_field_name() {
        // `code` is not in our URL redaction list (it's a one-time auth code,
        // already exchanged before any logging happens). Verify we don't accidentally
        // redact unrelated short identifiers.
        let body = "user_id=12345&error_code=invalid_grant";
        let out = redact_secrets(body);
        assert!(out.contains("12345"));
        assert!(out.contains("invalid_grant"));
    }

    // ── AI provider key redaction (security audit 2026-05-12) ──────────

    // Test fixtures use OBVIOUSLY_FAKE_TEST tokens so GitHub Push
    // Protection / TruffleHog don't flag them as real secrets. The
    // regex matches them just the same — they pass the length and
    // prefix shape requirements.

    #[test]
    fn redacts_openrouter_key_in_sdk_error_string() {
        // OpenAI SDK errors propagate as `openai.AuthenticationError:
        // Incorrect API key provided: sk-or-v1-...` which previously went
        // through `log::error!` un-scrubbed because the key wasn't in a
        // recognised field name.
        let body = "openai.AuthenticationError: Incorrect API key provided: sk-or-v1-OBVIOUSLY_FAKE_TEST_FIXTURE_NOT_A_REAL_KEY_AAAAAAAAAAAAAAAA";
        let out = redact_secrets(body);
        assert!(!out.contains("sk-or-v1-OBVIOUSLY"));
        assert!(out.contains("[REDACTED_AI_KEY]"));
    }

    #[test]
    fn redacts_anthropic_key_in_log_line() {
        let body = "Auth error sk-ant-OBVIOUSLY_FAKE_TEST_FIXTURE_NOT_REAL_AAAAAA";
        let out = redact_secrets(body);
        assert!(!out.contains("sk-ant-OBVIOUSLY"));
        assert!(out.contains("[REDACTED_AI_KEY]"));
    }

    #[test]
    fn redacts_openai_project_key() {
        let body = "Error: bad token sk-proj-OBVIOUSLY_FAKE_TEST_FIXTURE_NOT_REAL";
        let out = redact_secrets(body);
        assert!(!out.contains("sk-proj-OBVIOUSLY"));
        assert!(out.contains("[REDACTED_AI_KEY]"));
    }

    #[test]
    fn redacts_gemini_aiza_key() {
        let body = "Error: invalid key AIzaOBVIOUSLY_FAKE_TEST_FIXTURE_NOT_REAL_AAA";
        let out = redact_secrets(body);
        assert!(!out.contains("AIzaOBVIOUSLY"));
        assert!(out.contains("[REDACTED_AI_KEY]"));
    }

    #[test]
    fn ai_key_min_length_avoids_false_positives_on_short_tokens() {
        // Short tokens like `sk-test` (test fixture) shouldn't trigger
        // the AI key redaction — they're below the 20-char threshold
        // after the prefix.
        let body = "Test fixture used sk-test for unit testing";
        let out = redact_secrets(body);
        assert!(out.contains("sk-test"), "short token must NOT be redacted");
    }

    #[test]
    fn ai_key_redaction_chains_with_json_field_redaction() {
        // A real provider error body has the key in a recognised field
        // AND echoes it again in the message — both must be scrubbed.
        // Fixtures kept obviously fake to avoid GitHub Push Protection.
        let body = r#"{"error":{"message":"Invalid API key: sk-or-v1-OBVIOUSLY_FAKE_TOKEN_ONE_NOT_REAL_AAAAAAAAAAAAAAAAAAAAAA","code":"invalid_api_key"},"access_token":"sk-or-v1-OBVIOUSLY_FAKE_TOKEN_TWO_NOT_REAL_BBBBBBBBBBBBBBBBBBBBBB"}"#;
        let out = redact_secrets(body);
        // Both occurrences gone — exact tokens may differ in redact label.
        assert!(!out.contains("OBVIOUSLY_FAKE_TOKEN_ONE"));
        assert!(!out.contains("OBVIOUSLY_FAKE_TOKEN_TWO"));
        assert!(
            out.contains("[REDACTED]") || out.contains("[REDACTED_AI_KEY]"),
            "must contain at least one redaction marker"
        );
    }

    #[test]
    fn ai_key_redaction_preserves_safe_context() {
        let body = "Rate limit hit at 18:42 UTC, retry in 60s — no key in this message";
        let out = redact_secrets(body);
        // Nothing matches the AI key pattern.
        assert!(out.contains("Rate limit hit"));
        assert!(out.contains("60s"));
    }
}
