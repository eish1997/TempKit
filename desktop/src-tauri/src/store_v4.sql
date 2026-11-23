CREATE TABLE tasks(task_id TEXT PRIMARY KEY,submission_id TEXT NOT NULL UNIQUE,submission_json TEXT NOT NULL,task_json TEXT NOT NULL,card_json TEXT NOT NULL);
CREATE TABLE attempts(task_id TEXT NOT NULL REFERENCES tasks(task_id),attempt INTEGER NOT NULL CHECK(attempt>0),state TEXT NOT NULL,event_version INTEGER NOT NULL,PRIMARY KEY(task_id,attempt));
INSERT INTO schema_migrations VALUES(4,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=4;
