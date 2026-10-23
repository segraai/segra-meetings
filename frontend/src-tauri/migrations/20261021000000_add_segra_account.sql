-- Segra account for the "segra" summary provider: an API key from device sign-in and the account it
-- belongs to. The key can only start short-lived model sessions; transcripts go straight to the model.
ALTER TABLE settings ADD COLUMN segraApiKey TEXT;
ALTER TABLE settings ADD COLUMN segraUserId TEXT;
