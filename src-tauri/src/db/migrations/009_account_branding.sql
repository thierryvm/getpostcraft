-- Per-account branding for persona-agnostic prompts and image templates.
-- Both nullable: accounts created before this migration fall back to app defaults.
ALTER TABLE accounts ADD COLUMN brand_color TEXT;
ALTER TABLE accounts ADD COLUMN accent_color TEXT;
