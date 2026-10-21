# 0006. Activity and recordings live in SQLite (sqlx); settings in JSON files

## Status

Accepted (2026-10-21)

## Context

The tracker writes a row every time the foreground window changes: tens of thousands of rows a month, years
of them. The Day view must open instantly, reports span months, and the data must live in one folder the
user can see, move and back up. DataBee keeps configuration in JSON files through `JsonStore`, which is
simple and robust but loads whole files into memory.

## Decision

Activity, away spans, recordings, projects, clients, tags, rules and saved reports live in one SQLite file,
`data/timetracker.sqlite`, opened through sqlx 0.9 (`sqlite`, `runtime-tokio`, `migrate`) in WAL mode with
`synchronous = NORMAL`, foreign keys on and a small pool (SQLite has one writer). The schema is versioned with
`sqlx::migrate!` and embedded SQL files. Settings, keybindings and UI state stay in JSON files through
DataBee's `JsonStore`, because they are small, human-editable and must load before the database exists.
rusqlite was rejected only for consistency with DataBee's sqlx usage; JSON per day was rejected because range
reports and "years of data" would read everything.

## Consequences

- Day and range queries are indexed; the Day view reads one day's rows.
- The data folder is one file plus WAL side files; a backup is a JSON export or a copy while the app is closed.
- Migrations are forward-only; a downgrade restores a backup.
- sqlx's `sqlite` feature pulls in `libsqlite3-sys` with bundled SQLite and unsafe code.
