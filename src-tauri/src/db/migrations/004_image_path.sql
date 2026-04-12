-- Add image path and Instagram media ID to post history
ALTER TABLE post_history ADD COLUMN image_path TEXT;
ALTER TABLE post_history ADD COLUMN ig_media_id TEXT;
