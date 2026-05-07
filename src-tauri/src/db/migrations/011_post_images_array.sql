-- Multi-image support for carousel posts (Instagram CAROUSEL, LinkedIn gallery).
--
-- Existing schema only carried a single `image_path` per post — fine for single-image
-- IG posts but loses every slide except the first when the user generates a carousel.
-- Add an `images` JSON-array column carrying ALL slides; readers pick the right
-- branch (single vs carousel) at publish time.
--
-- `image_path` is kept as a derived/legacy column so existing readers keep working
-- during the transition. Writers update both: `images = JSON array`, `image_path
-- = images[0]` for backward compatibility.
ALTER TABLE post_history ADD COLUMN images TEXT;

-- Backfill: any post that already has a single image_path becomes a 1-image array.
-- Posts without an image (text-only LinkedIn drafts) keep images = NULL.
-- Manual JSON construction since older SQLite versions may lack the json1 extension.
-- Escapes: replace " with \" inside the value to keep the array valid.
UPDATE post_history
SET images = '["' || REPLACE(image_path, '"', '\"') || '"]'
WHERE image_path IS NOT NULL AND images IS NULL;
