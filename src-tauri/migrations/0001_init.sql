-- Forward-only: never edit this file once released; add NNNN_<name>.sql instead.
-- Every timestamp is UTC milliseconds.

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE clients (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  archived_at INTEGER,
  created_at INTEGER NOT NULL
);

CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  client_id TEXT REFERENCES clients(id),
  name TEXT NOT NULL,
  color TEXT NOT NULL,             -- an accent name
  billable INTEGER NOT NULL DEFAULT 1,
  hourly_rate REAL,
  archived_at INTEGER,
  created_at INTEGER NOT NULL
);

CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  created_at INTEGER NOT NULL
);

-- AUTOINCREMENT: ids are never reused, so an undo can restore deleted rows by id.
CREATE TABLE activities (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL,            -- 'tracker' or 'external:<name>'
  start INTEGER NOT NULL,
  "end" INTEGER,                   -- NULL while the span is open
  app TEXT NOT NULL,
  bundle_id TEXT,
  title TEXT NOT NULL DEFAULT '',
  url TEXT,
  path TEXT,
  private INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX activities_start ON activities(start);
CREATE INDEX activities_source_start ON activities(source, start);

CREATE TABLE away_spans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  start INTEGER NOT NULL,
  "end" INTEGER,
  reason TEXT NOT NULL             -- idle | sleep | lock | paused
);
CREATE INDEX away_start ON away_spans(start);

CREATE TABLE recordings (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id),
  start INTEGER NOT NULL,
  "end" INTEGER,                   -- NULL only for the running timer
  note TEXT NOT NULL DEFAULT '',
  billable INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX recordings_start ON recordings(start);

CREATE TABLE recording_tags (
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (recording_id, tag_id)
);

INSERT INTO meta (key, value) VALUES
  ('schema', '1'),
  ('created_at', CAST(CAST(unixepoch('subsec') * 1000 AS INTEGER) AS TEXT));
