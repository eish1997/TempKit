CREATE TABLE scenes(scene_id TEXT PRIMARY KEY,revision INTEGER NOT NULL CHECK(revision>=0),scene_json TEXT NOT NULL);
CREATE TABLE result_placements(task_id TEXT NOT NULL,attempt INTEGER NOT NULL,resource_id TEXT NOT NULL REFERENCES resources(resource_id),placement_id TEXT NOT NULL,PRIMARY KEY(task_id,attempt,resource_id),FOREIGN KEY(task_id,attempt) REFERENCES attempts(task_id,attempt));
CREATE TABLE task_events(sequence INTEGER PRIMARY KEY AUTOINCREMENT,task_json TEXT NOT NULL);
INSERT INTO task_events(task_json) SELECT task_json FROM tasks ORDER BY rowid;
CREATE TRIGGER task_event_insert AFTER INSERT ON tasks BEGIN INSERT INTO task_events(task_json) VALUES(NEW.task_json); END;
CREATE TRIGGER task_event_update AFTER UPDATE OF task_json ON tasks WHEN NEW.task_json!=OLD.task_json BEGIN INSERT INTO task_events(task_json) VALUES(NEW.task_json); END;
INSERT INTO schema_migrations VALUES(6,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=6;
