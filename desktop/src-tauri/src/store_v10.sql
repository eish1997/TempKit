CREATE TABLE gateway_checks (
 connection_id TEXT PRIMARY KEY REFERENCES gateway_connections(connection_id),
 models_json TEXT NOT NULL
);
CREATE TABLE image_bindings (
 connection_id TEXT NOT NULL REFERENCES gateway_connections(connection_id),
 capability_id TEXT NOT NULL,
 revision TEXT NOT NULL,
 binding_json TEXT NOT NULL,
 PRIMARY KEY(connection_id,capability_id,revision)
);
CREATE TRIGGER image_binding_no_update BEFORE UPDATE ON image_bindings
BEGIN SELECT RAISE(ABORT,'image capability revisions are immutable'); END;
CREATE TRIGGER image_binding_no_delete BEFORE DELETE ON image_bindings
BEGIN SELECT RAISE(ABORT,'historical image capability revisions must be retained'); END;
INSERT INTO schema_migrations VALUES(10,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=10;
CREATE TABLE image_task_bindings (
 task_id TEXT PRIMARY KEY REFERENCES tasks(task_id),
 binding_json TEXT NOT NULL
);
CREATE TRIGGER image_task_binding_no_update BEFORE UPDATE ON image_task_bindings
BEGIN SELECT RAISE(ABORT,'task execution bindings are immutable'); END;
CREATE TRIGGER image_task_binding_no_delete BEFORE DELETE ON image_task_bindings
BEGIN SELECT RAISE(ABORT,'task execution bindings must be retained'); END;
