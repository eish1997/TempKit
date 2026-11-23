CREATE TABLE image_cost_scopes (
 approval_id TEXT PRIMARY KEY REFERENCES image_cost_approvals(id),
 rule_json TEXT NOT NULL
);
CREATE TRIGGER image_cost_scope_no_update BEFORE UPDATE ON image_cost_scopes BEGIN SELECT RAISE(ABORT,'cost scope is immutable'); END;
CREATE TRIGGER image_cost_scope_no_delete BEFORE DELETE ON image_cost_scopes BEGIN SELECT RAISE(ABORT,'cost scope must be retained'); END;
INSERT INTO schema_migrations VALUES(14,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
PRAGMA user_version=14;
