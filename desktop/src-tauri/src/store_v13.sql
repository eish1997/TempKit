CREATE TABLE gateway_credential_rotations (
 connection_id TEXT NOT NULL REFERENCES gateway_connections(connection_id),
 revision INTEGER NOT NULL CHECK(revision > 0),
 credential_ref TEXT NOT NULL UNIQUE,
 previous_ref TEXT NOT NULL,
 PRIMARY KEY(connection_id,revision)
);
CREATE TRIGGER gateway_rotation_no_update BEFORE UPDATE ON gateway_credential_rotations
BEGIN SELECT RAISE(ABORT,'credential rotation history is immutable'); END;
CREATE TRIGGER gateway_rotation_no_delete BEFORE DELETE ON gateway_credential_rotations
BEGIN SELECT RAISE(ABORT,'credential rotation history must be retained'); END;
INSERT INTO schema_migrations VALUES(13,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=13;
