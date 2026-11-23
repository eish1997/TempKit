CREATE TABLE pending_task_outputs(task_id TEXT NOT NULL,attempt INTEGER NOT NULL,bytes BLOB NOT NULL,mime TEXT NOT NULL,PRIMARY KEY(task_id,attempt),FOREIGN KEY(task_id,attempt) REFERENCES attempts(task_id,attempt));
ALTER TABLE attempts ADD COLUMN execution_count INTEGER NOT NULL DEFAULT 0;
CREATE TABLE task_failures(task_id TEXT NOT NULL,attempt INTEGER NOT NULL,stage TEXT NOT NULL,message TEXT NOT NULL,PRIMARY KEY(task_id,attempt),FOREIGN KEY(task_id,attempt) REFERENCES attempts(task_id,attempt));
INSERT INTO schema_migrations VALUES(5,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=5;
