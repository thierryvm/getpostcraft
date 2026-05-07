-- deepseek/deepseek-chat-v3-5:free no longer exists on OpenRouter (400 invalid model ID).
-- Reset to default model.
UPDATE settings
SET value = 'anthropic/claude-haiku-4-5-20251001'
WHERE key = 'active_model'
  AND value = 'deepseek/deepseek-chat-v3-5:free';
