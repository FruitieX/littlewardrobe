-- Store images and configurable prompts extend strict JSON contracts in existing rows.
-- Older binaries must reject this database instead of failing while reading those rows.
INSERT INTO schema_migrations(version,applied_at) VALUES(3,datetime('now'));
