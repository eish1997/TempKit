CREATE TABLE image_budget_releases (
 task_id TEXT NOT NULL,
 attempt INTEGER NOT NULL,
 reason TEXT NOT NULL CHECK(reason='confirmed_not_sent'),
 PRIMARY KEY(task_id,attempt),
 FOREIGN KEY(task_id,attempt) REFERENCES image_budget_reservations(task_id,attempt)
);
CREATE TRIGGER image_release_no_update BEFORE UPDATE ON image_budget_releases BEGIN SELECT RAISE(ABORT,'budget releases are immutable'); END;
CREATE TRIGGER image_release_no_delete BEFORE DELETE ON image_budget_releases BEGIN SELECT RAISE(ABORT,'budget releases must be retained'); END;
INSERT INTO schema_migrations VALUES(12,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=12;
