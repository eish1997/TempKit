CREATE TABLE migration_candidates(migration_id TEXT PRIMARY KEY REFERENCES migration_runs(migration_id),scene_json TEXT NOT NULL);
INSERT INTO schema_migrations VALUES(3,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=3;
