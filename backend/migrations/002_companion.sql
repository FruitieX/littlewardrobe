CREATE TABLE companion_workers(household_id TEXT PRIMARY KEY REFERENCES households(id), token_hash TEXT NOT NULL UNIQUE, last_seen INTEGER NOT NULL DEFAULT 0);
CREATE TABLE companion_jobs(id TEXT PRIMARY KEY, household_id TEXT NOT NULL REFERENCES households(id), path TEXT NOT NULL, body TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending', lease TEXT, result TEXT, deadline INTEGER NOT NULL);
CREATE INDEX companion_jobs_household ON companion_jobs(household_id,status,deadline);
INSERT INTO schema_migrations(version,applied_at) VALUES(2,datetime('now'));
