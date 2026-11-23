CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
CREATE TABLE blobs(blob_key TEXT PRIMARY KEY, digest TEXT NOT NULL, byte_length INTEGER NOT NULL CHECK(byte_length > 0));
CREATE TABLE resources(resource_id TEXT PRIMARY KEY, blob_key TEXT NOT NULL REFERENCES blobs(blob_key), metadata_json TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('available','missing','retired')));
CREATE TABLE resource_refs(owner_kind TEXT NOT NULL, owner_id TEXT NOT NULL, resource_id TEXT NOT NULL REFERENCES resources(resource_id), PRIMARY KEY(owner_kind,owner_id,resource_id));
INSERT INTO schema_migrations VALUES(1, strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=1;
