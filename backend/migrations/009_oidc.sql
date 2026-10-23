-- Optional OpenID Connect sign-in. Identities are keyed by issuer and subject;
-- sliding sessions extend themselves while in use.
CREATE TABLE IF NOT EXISTS oidc_identities(issuer TEXT NOT NULL, subject TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, username TEXT NOT NULL DEFAULT '', linked_at INTEGER NOT NULL, PRIMARY KEY(issuer,subject));
CREATE INDEX IF NOT EXISTS oidc_identities_user ON oidc_identities(user_id);
CREATE TABLE IF NOT EXISTS oidc_logins(state_hash TEXT PRIMARY KEY, nonce TEXT NOT NULL, verifier TEXT NOT NULL, expires_at INTEGER NOT NULL);
ALTER TABLE sessions ADD COLUMN sliding INTEGER NOT NULL DEFAULT 0;
INSERT INTO schema_migrations(version,applied_at) VALUES(9,datetime('now'));
