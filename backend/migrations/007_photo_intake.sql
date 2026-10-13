CREATE TABLE clothing_photo_jobs (
  id TEXT PRIMARY KEY,
  household_id TEXT NOT NULL REFERENCES households(id),
  garment_id TEXT NOT NULL,
  actor TEXT NOT NULL,
  baseline TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued',
  run_id TEXT,
  error TEXT,
  created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX clothing_photo_queue ON clothing_photo_jobs(status, created_at);
INSERT INTO schema_migrations(version,applied_at) VALUES(7,datetime('now'));
