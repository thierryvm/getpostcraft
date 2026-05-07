-- Update default model to Claude Haiku 4.5 (replaces claude-3-5-haiku).
UPDATE settings
SET value = 'anthropic/claude-haiku-4-5-20251001'
WHERE key = 'active_model'
  AND value IN (
    'anthropic/claude-3-5-haiku',
    'anthropic/claude-3-5-sonnet',
    'anthropic/claude-haiku-4-5',
    'anthropic/claude-sonnet-4-5'
  );
