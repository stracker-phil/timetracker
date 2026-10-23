-- Suggestions other apps store through the CLI or MCP (ADR 0028), reviewed in the Day view like a rule's.
-- `spans` is a JSON array of {start, end}, by start, all on the local `day`.
CREATE TABLE stored_suggestions (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id),
  note TEXT NOT NULL,
  tag_id TEXT REFERENCES tags(id) ON DELETE SET NULL,
  source TEXT NOT NULL,
  day TEXT NOT NULL,
  spans TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX stored_suggestions_day ON stored_suggestions(day);
