-- The historical bad-pool cleanup can scan many samples, so a persisted marker
-- guarantees the repair runs once after deployment instead of on every restart.
CREATE TABLE IF NOT EXISTS maintenance_flags (
    key         TEXT PRIMARY KEY,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
