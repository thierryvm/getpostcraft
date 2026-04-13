-- Reset deprecated OpenRouter free models to the default.
-- These models were removed from OpenRouter and return 404.
UPDATE settings
SET value = 'anthropic/claude-3-5-haiku'
WHERE key = 'active_model'
  AND value IN (
    'mistralai/mistral-7b-instruct:free',
    'meta-llama/llama-3.2-3b-instruct:free',
    'qwen/qwen-2.5-7b-instruct:free',
    'deepseek/deepseek-r1:free'
  );
