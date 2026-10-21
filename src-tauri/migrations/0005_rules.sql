-- Rules suggest recordings for matching activity; the lowest `position` wins. `tag_ids` is a JSON array.
CREATE TABLE rules (id TEXT PRIMARY KEY, name TEXT NOT NULL, project_id TEXT NOT NULL REFERENCES projects(id),
  position INTEGER NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, note TEXT NOT NULL DEFAULT '',
  tag_ids TEXT NOT NULL DEFAULT '[]');
CREATE TABLE rule_conditions (id TEXT PRIMARY KEY, rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
  position INTEGER NOT NULL, field TEXT NOT NULL, op TEXT NOT NULL, value TEXT NOT NULL);
