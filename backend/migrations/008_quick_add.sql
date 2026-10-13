-- Keep the accepted request immutable across retries so filter defaults cannot
-- be changed by reusing a request ID. Existing jobs have no explicit defaults.
ALTER TABLE clothing_photo_jobs ADD COLUMN request TEXT NOT NULL DEFAULT '{}';
INSERT INTO schema_migrations(version,applied_at) VALUES(8,datetime('now'));
