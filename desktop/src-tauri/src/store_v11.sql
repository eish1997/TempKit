CREATE TABLE image_budget (
 id INTEGER PRIMARY KEY CHECK(id=1),
 limit_micros INTEGER NOT NULL CHECK(limit_micros>0)
);
CREATE TABLE image_cost_approvals (
 id TEXT PRIMARY KEY,
 connection_id TEXT NOT NULL,
 capability_id TEXT NOT NULL,
 revision TEXT NOT NULL,
 ceiling_micros INTEGER NOT NULL CHECK(ceiling_micros>0),
 expires_at_s INTEGER NOT NULL,
 FOREIGN KEY(connection_id,capability_id,revision) REFERENCES image_bindings(connection_id,capability_id,revision)
);
CREATE TABLE image_budget_reservations (
 task_id TEXT NOT NULL,
 attempt INTEGER NOT NULL,
 approval_id TEXT NOT NULL REFERENCES image_cost_approvals(id),
 amount_micros INTEGER NOT NULL CHECK(amount_micros>0),
 PRIMARY KEY(task_id,attempt),
 FOREIGN KEY(task_id,attempt) REFERENCES attempts(task_id,attempt)
);
CREATE TRIGGER image_budget_no_delete BEFORE DELETE ON image_budget BEGIN SELECT RAISE(ABORT,'budget cannot be reset'); END;
CREATE TRIGGER image_budget_no_update BEFORE UPDATE ON image_budget BEGIN SELECT RAISE(ABORT,'budget limit is immutable'); END;
CREATE TRIGGER image_cost_no_update BEFORE UPDATE ON image_cost_approvals BEGIN SELECT RAISE(ABORT,'cost approvals are immutable'); END;
CREATE TRIGGER image_cost_no_delete BEFORE DELETE ON image_cost_approvals BEGIN SELECT RAISE(ABORT,'cost approvals must be retained'); END;
CREATE TRIGGER image_reservation_no_update BEFORE UPDATE ON image_budget_reservations BEGIN SELECT RAISE(ABORT,'budget reservations are immutable'); END;
CREATE TRIGGER image_reservation_no_delete BEFORE DELETE ON image_budget_reservations BEGIN SELECT RAISE(ABORT,'budget reservations must be retained'); END;
INSERT INTO schema_migrations VALUES(11,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=11;
