-- A stored suggestion may have no project yet (ADR 0030): the owner picks one before approving it.
-- SQLite can't drop a NOT NULL, so the table is rebuilt; nothing references it.
CREATE TABLE stored_suggestions_new (
  id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(id),
  note TEXT NOT NULL,
  tag_id TEXT REFERENCES tags(id) ON DELETE SET NULL,
  source TEXT NOT NULL,
  day TEXT NOT NULL,
  spans TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
INSERT INTO stored_suggestions_new SELECT id, project_id, note, tag_id, source, day, spans, created_at, updated_at
  FROM stored_suggestions;
DROP TABLE stored_suggestions;
ALTER TABLE stored_suggestions_new RENAME TO stored_suggestions;
CREATE INDEX stored_suggestions_day ON stored_suggestions(day);
