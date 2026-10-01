ALTER TABLE uploads ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'original';
INSERT INTO schema_migrations(version,applied_at) VALUES(6,datetime('now'));
