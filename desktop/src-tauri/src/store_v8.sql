CREATE TABLE gateway_profiles (
    profile_id TEXT PRIMARY KEY,
    profile_json TEXT NOT NULL
);
CREATE TRIGGER gateway_profile_no_update BEFORE UPDATE ON gateway_profiles
BEGIN SELECT RAISE(ABORT,'gateway profiles are immutable'); END;
CREATE TRIGGER gateway_profile_no_delete BEFORE DELETE ON gateway_profiles
BEGIN SELECT RAISE(ABORT,'historical gateway profiles must be retained'); END;
INSERT INTO schema_migrations VALUES(8,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=8;
