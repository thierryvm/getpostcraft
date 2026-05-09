-- Migration 017 — published_url on post_history.
--
-- Why: `ig_media_id` stores Meta's numeric media id (e.g. "17905614200109834")
-- and LinkedIn's URN (e.g. "urn:li:share:7458945576970178560"). LinkedIn URNs
-- deep-link directly via /feed/update/{urn}/, but Instagram's media id is
-- NOT a URL component — the public post URL uses an opaque shortcode
-- (e.g. "DYCzSnDDeeH") that's only obtainable via the Graph API
-- `?fields=permalink` endpoint after publish.
--
-- Without this column the "Voir sur Instagram" button can only link to the
-- account's profile feed, where the just-published post happens to sit at
-- the top. Adding `published_url` lets us deep-link to the exact post like
-- LinkedIn does.
--
-- Nullable on purpose: legacy rows (published before this migration) and
-- LinkedIn rows (which derive their URL from `ig_media_id` instead) leave
-- this column NULL. The frontend prefers `published_url` when set, falls
-- back to the URN-derived or profile URL otherwise.

ALTER TABLE post_history ADD COLUMN published_url TEXT;
