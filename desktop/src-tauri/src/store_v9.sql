CREATE TABLE gateway_connections (
 connection_id TEXT PRIMARY KEY,
 profile_id TEXT NOT NULL REFERENCES gateway_profiles(profile_id),
 connection_json TEXT NOT NULL,
 revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1))
);
CREATE TRIGGER gateway_connection_identity BEFORE UPDATE OF connection_id,profile_id,connection_json ON gateway_connections
BEGIN SELECT RAISE(ABORT,'connection versions are immutable'); END;
CREATE TRIGGER gateway_connection_no_delete BEFORE DELETE ON gateway_connections
BEGIN SELECT RAISE(ABORT,'historical connections must be retained'); END;
CREATE TRIGGER gateway_connection_no_reactivate BEFORE UPDATE OF revoked ON gateway_connections
WHEN OLD.revoked=1 AND NEW.revoked=0
BEGIN SELECT RAISE(ABORT,'revoked connections cannot be reactivated'); END;
INSERT INTO schema_migrations VALUES(9,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=9;
