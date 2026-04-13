-- Add product_truth column to accounts.
-- Nullable: accounts created before this migration keep NULL until the user fills it in.
ALTER TABLE accounts ADD COLUMN product_truth TEXT;
