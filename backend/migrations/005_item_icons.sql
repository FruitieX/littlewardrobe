CREATE TABLE item_icon_jobs(
    household_id TEXT NOT NULL REFERENCES households(id),
    product_id TEXT NOT NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    run_id TEXT,
    error TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    started_at INTEGER,
    PRIMARY KEY(household_id,product_id)
);
CREATE INDEX item_icon_jobs_pending ON item_icon_jobs(status,created_at);
CREATE TABLE item_icon_attempts(
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES households(id),
    started_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX item_icon_attempts_recent ON item_icon_attempts(household_id,started_at);
INSERT INTO schema_migrations(version,applied_at) VALUES(5,datetime('now'));
