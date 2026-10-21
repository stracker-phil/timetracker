-- Report filters the user keeps; `filter` is a `ReportFilter` as JSON, `position` the sidebar order.
CREATE TABLE saved_reports (id TEXT PRIMARY KEY, name TEXT NOT NULL, filter TEXT NOT NULL, position INTEGER NOT NULL);
