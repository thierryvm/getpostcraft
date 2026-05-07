-- Fix OpenRouter model IDs that used the dash-separated Anthropic-native format
-- (e.g. anthropic/claude-haiku-4-5-20251001) — OpenRouter rejects these with a
-- 400 "is not a valid model ID". OpenRouter's actual slugs use dots:
-- anthropic/claude-haiku-latest, anthropic/claude-sonnet-4.6, claude-opus-4.7.
--
-- Source verified 2026-05-07 via https://openrouter.ai/api/v1/models.
--
-- We reset only the broken IDs and leave anything else untouched so users who
-- picked a still-valid model (gpt-4o-mini, deepseek, gemini, ollama) keep their
-- preference. The new default `claude-sonnet-4.6` matches the recommended
-- baseline (top quality, ~$0.30/month at 30 posts).
UPDATE settings
SET value = 'anthropic/claude-sonnet-4.6'
WHERE key = 'active_model'
  AND value IN (
    'anthropic/claude-haiku-4-5-20251001',
    'anthropic/claude-haiku-4-5',
    'anthropic/claude-sonnet-4-6',
    'anthropic/claude-opus-4-6',
    'anthropic/claude-3-5-haiku',
    'anthropic/claude-3-5-sonnet'
  );
