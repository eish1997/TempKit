CREATE TABLE migration_runs(
 migration_id TEXT PRIMARY KEY, source_digest TEXT NOT NULL UNIQUE,
 snapshot_json TEXT NOT NULL, inventory_json TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('created','copying','verified','active')),
 last_error TEXT
);
CREATE TABLE migration_id_maps(
 migration_id TEXT NOT NULL REFERENCES migration_runs(migration_id),
 legacy_key TEXT NOT NULL, source_digest TEXT NOT NULL,
 resource_id TEXT NOT NULL REFERENCES resources(resource_id),
 PRIMARY KEY(migration_id,legacy_key)
);
INSERT INTO schema_migrations VALUES(2,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=2;
