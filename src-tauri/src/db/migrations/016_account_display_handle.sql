-- Migration 016 — display_handle on accounts.
--
-- Why: LinkedIn's OAuth profile endpoint returns the user's full personal
-- name (e.g. "Thierry Vanmeeteren") as the username field. The carousel
-- brand-stamp template renders `>_ @{username}` directly, so a LinkedIn
-- post on @terminallearning's brand carries the owner's personal name on
-- every slide — not the desired brand handle.
--
-- This column lets the user pick a display handle separately from the
-- platform-supplied username. Nullable: when set, the renderer prefers
-- it; when NULL, fall back to `username` (preserves Instagram's
-- handle-style usernames without forcing a re-config).

ALTER TABLE accounts ADD COLUMN display_handle TEXT;
