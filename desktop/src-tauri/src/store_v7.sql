CREATE TABLE notes (
    note_id TEXT PRIMARY KEY,
    note_json TEXT NOT NULL
);
INSERT INTO schema_migrations VALUES(7,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=7;
