-- Hidden is separate from ignored: ignored controls Telegram ingestion, while
-- hidden only removes old/noisy ignored channels from the default UI list.
ALTER TABLE channels ADD COLUMN IF NOT EXISTS is_hidden BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS channels_hidden_idx ON channels(is_ignored, is_hidden, name);
