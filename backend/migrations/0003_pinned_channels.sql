-- Pinned channels are a personal UI preference. Keeping the flag in Postgres
-- makes the ordering stable across browser restarts and future desktop builds.
ALTER TABLE channels ADD COLUMN IF NOT EXISTS is_pinned BOOLEAN NOT NULL DEFAULT FALSE;
