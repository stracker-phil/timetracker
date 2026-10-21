# Data store

All user data lives in one SQLite file, opened through sqlx ([ADR 0006](adr/0006-sqlite-for-data-json-for-settings.md)).
Settings, keybindings and UI state stay in JSON files ([backend.md](backend.md#persistence)). The code is in
`src-tauri/src/db/`.

## The data folder

`db::data_dir(settings, config_dir)` is `dataDir` from `settings.json` when it is an absolute path, else
`<config dir>/data`, so by default:

```
~/Library/Application Support/com.philippstracker.timetracker/data/
  timetracker.sqlite   timetracker.sqlite-wal   timetracker.sqlite-shm
```

`Db::open(path)` creates the folder (mode 0700) and the file when missing, then connects with WAL journaling,
`synchronous = NORMAL`, a 5 s busy timeout and foreign keys on, through a pool of 4 connections (SQLite has one
writer; readers run beside it in WAL). Then it runs the migrations and seeds the default tags.

`AppState.data` is a `DataStore`: the file path plus the open `Db` or the reason it could not be opened, behind
an `RwLock` so the folder can move later. `AppState::open` opens it before Tauri starts
(`tauri::async_runtime::block_on`). When opening fails, the app logs it and runs without a database:
`DataStore::db()` returns `db` "The data store is not available", and Settings → Data shows the error. On
`RunEvent::Exit` the pool closes.

A backup is the JSON export below, or a copy of the folder while the app is not running.

## Schema

`migrations/0001_init.sql`. Booleans are `INTEGER` 0/1; every timestamp is UTC milliseconds.

| Table | Holds |
|---|---|
| `meta` | `key`/`value` facts: `schema` (`1`), `created_at`, `tags_seeded` |
| `clients` | `id` (UUID), `name`, `color` (an accent name), `archived_at`, `created_at` |
| `projects` | `id` (UUID), `client_id`, `name`, `color` (the client's accent, or its own without a client), `shade` (`0..SHADES`), `archived_at`, `created_at` |
| `tags` | `id` (UUID), `name` (unique, ignoring case), `created_at` |
| `activities` | one span in one app and window: `id` (AUTOINCREMENT), `source` (`tracker` or `external:<name>`), `start`, `end` (NULL while open), `app`, `bundle_id`, `title`, `url`, `path`, `private`; indexed on `start` and `(source, start)` |
| `away_spans` | no input, sleep, lock or pause: `id` (AUTOINCREMENT), `start`, `end`, `reason` (`idle`, `sleep`, `lock`, `paused`); indexed on `start` |
| `recordings` | time confirmed for a project: `id` (UUID), `project_id`, `start`, `end` (NULL only for the running timer), `note`, `created_at`, `updated_at`; indexed on `start` |
| `recording_tags` | `(recording_id, tag_id)`; deleting either side deletes the link |

`end` is a SQL keyword, so queries quote it (`"end"`). `activities` and `away_spans` use AUTOINCREMENT so a
deleted id is never reused and an undo can restore rows by id.

### Data rules

- Activity and recordings are separate layers ([ADR 0005](adr/0005-activity-and-recordings-are-separate-layers.md)):
  nothing turns activity into recordings on its own.
- Activity and away rows are never edited, except to set `end` on the open span. Deleting activity is an
  explicit user action and deletes crossing rows whole.
- Spans are stored unsplit. A reader clips them to the range it shows, so a span across midnight shows on
  both days.
- Recordings never overlap each other; the running timer is the one recording without `end`. The recordings
  feature enforces this; the repository only offers the `overlapping` query.
- A note is one line ([ADR 0018](adr/0018-one-line-notes-tags-copyable-reports.md)).

### External activity

Other apps write activity through the CLI, `timetracker://` links or the `activity_*` commands
([ADR 0009](adr/0009-external-activity-with-a-source.md), [backend.md](backend.md#cli-and-single-instance-handoff)).
`activity/external.rs` holds the rules; `activity/mod.rs` adds the `data_changed` events.

- **Source.** `ActivityInput.source` is a name of 1 to 32 lowercase letters, digits, `-` and `_`, starting
  with a letter or digit; it is stored as `external:<name>`, and `tracker` is refused. Each source is its own
  lane in the Day view.
- **Fields** (`validate`, `invalid_input` naming the field): `title` is trimmed and not blank, at most 500
  characters; `app` (default: the source name) at most 500; `url` and `path` at most 2,000, blank ones stored
  as `NULL`. Rows get `bundle_id = NULL` and `private = 0`.
- **Times.** `end > start`, at most 24 h apart; `start` at most 60 s ahead of now (another app's clock may
  run ahead). An `end` in the future is allowed, so a planned meeting can be written ahead.
- **Overlap.** A span must not share time with a row of the **same source** (`overlap`, the message lists
  the row ids, `details.ids` holds them); touching edges are fine. Other sources and the tracker may overlap.
- **`add_many`** writes closed spans (each needs `end`); it validates every item, then inserts all in one
  transaction, items checked in order against the rows before them: one bad item fails the batch. In a batch
  the error names the item (`items[2].title`).
- **`start`** opens a span (`end = NULL`) and closes the source's open span at the new start, refusing a
  start before that span's start: at most one span per source is open. **`stop`** closes it at `end` (default
  now, at most 24 h ahead) and returns `None` when nothing is open; a span may stay open for longer than
  24 h, so `stop` does not limit its length.
- The tracker never touches external rows; startup recovery leaves them open
  ([capture.md](capture.md#startup-and-the-loop)).

## Time

`db/time.rs` ([ADR 0011](adr/0011-utc-milliseconds-local-days.md)):

- `Ms` (`i64`) is UTC milliseconds; `now_ms()`.
- `Day` is a local calendar day. It holds a date and crosses IPC as the string `YYYY-MM-DD`; deserializing
  validates it, `Day::parse` rejects anything but exactly `YYYY-MM-DD` naming a real date (`invalid_input`).
- `Day::today()`, `Day::of(ms)`, `prev`, `next`, `Day::range(from, to)` (inclusive), `week_start` (Monday),
  `month_start`.
- `Day::bounds()` is `[start, end)` from local midnight to the next one, so DST days are 23 or 25 h. A
  skipped midnight (a DST gap at 00:00) takes the first valid local time after it; an ambiguous one takes the
  earlier instant. `bounds_in(&tz)` and `of_in(ms, &tz)` take the zone as a parameter, so tests use fixed
  zones (`chrono-tz`, dev only) and never the machine's.
- `clip(start, end, bounds, now)` is the part of a span inside `bounds`; an open span ends at `now`; `None`
  when nothing is inside.

The page never computes day bounds or week starts; it only formats.

## Repositories

One module per table, each with plain `async fn …(db: &Db, …) -> AppResult<…>` functions and typed rows
(`sqlx::FromRow`, `Serialize`, `TS`, camelCase). Queries use `sqlx::query`/`query_as` with bind parameters;
there are no `query!` macros (they need a database at compile time).

| Module | Functions | Invariants |
|---|---|---|
| `clients.rs` | `list(include_archived)`, `get`, `insert(name)`, `update(id, name)`, `set_archived(id, bool)` | names unique (archived clients count); lists by name ignoring case; archiving keeps the first `archived_at` and archives the client's projects in the same transaction; restoring restores the client only |
| `projects.rs` | `list`, `list_items(include_archived, now)`, `get`, `insert(ProjectInput)`, `update(id, ProjectInput)`, `set_archived`, `recent(limit)`, `resolve(name_or_id)`, `count` | see [Projects, clients and tags](#projects-clients-and-tags) |
| `tags.rs` | `list`, `list_with_usage`, `insert(name)`, `rename(id, name)`, `delete(id)`, `resolve_names(names)`, `seed_defaults` | names unique ignoring case: `insert` of a taken name returns the existing tag, `rename` to it is `invalid_input`; `DEFAULT_TAGS` are seeded once per data store (`meta.tags_seeded`), so deleting them all does not bring them back |
| `activities.rs` | `apps_seen(from, now)`, `list_clipped(from, to, now)`, `close_stale(source, duration)`, `replace_open(id, NewActivity)`, `insert(NewActivity) -> id`, `close_open(source, end)`, `open_by_source`, `overlapping(source, start, end)`, `sources_with_prefix(prefix)` (latest first); `insert_on`, `get_on`, `close_open_on`, `open_by_source_on`, `overlapping_on` take a connection so a caller can combine them in a transaction; `list_for_range(from, to)`, `list_sources(from, to)`, `delete_range(from, to)`, `count`, `first_start` | ranges are `[from, to)` and include open spans; touching edges do not overlap; results are not clipped; `close_open` never sets an end before the start |
| `away.rs` | `insert(NewAway) -> id`, `close_open(end)`, `open`, `list_for_range`, `delete_range` | as for activities; `AwayReason` is stored and sent as `idle`, `sleep`, `lock`, `paused` |
| `recordings.rs` | `insert(NewRecording)`, `get`, `update(id, RecordingPatch)`, `delete(id) -> Recording`, `list_for_range`, `running`, `overlapping(start, end, except)`, `count` | one transaction per write, tags included; `tag_ids` ordered by tag name; the project and every tag must exist (`not_found`); a note with a line break is `invalid_input`, notes are stored trimmed; `end` must be after `start`; `delete` returns the row so it can be restored under its id |
| `backup.rs` | `export(db) -> Backup`, `write_to(db, path)` | every table, read in one transaction |

`RecordingPatch` leaves absent fields as they are; `end: null` makes a recording running (`Option<Option<Ms>>`).

Errors: `From<sqlx::Error>` maps a database error to `db` with SQLite's extended result code as `code`
(`2067` unique, `787` foreign key), `RowNotFound` to `not_found`, anything else to `db`; a failed migration is
`db` "Migration failed: …".

### Projects, clients and tags

Validation errors are `invalid_input` with `field` set (`AppError::InvalidField`), so a form shows them under
the field:

| Field | Rule |
|---|---|
| `name` | trimmed, not blank, at most 100 characters (`db::MAX_NAME_CHARS`); unique ignoring case (Unicode, not only ASCII): clients among all clients, projects among the projects of the same client (or among client-less ones), tags among all tags. Archived rows count |
| `color` (client) | one of `ACCENTS`; blank: a new client takes the first accent no active client has, an edit keeps its color. A new color moves the client's projects to it |
| `color` (project) | a project of a client takes the client's color, whatever the input; without a client one of `ACCENTS`, blank becomes `orange` (`projects::DEFAULT_COLOR`) |
| `shade` | `0..SHADES` (9 lightness steps of the color); `null` keeps the shade on an edit within the same client, else takes the first of `SHADE_ORDER` no sibling has (projects of the same client, or all client-less ones) |
| `clientId` | must exist (`not_found`) and not be archived, checked on create and when the client changes, so a project of an archived client stays editable |

- Nothing is deleted except tags: recordings reference projects, so projects and clients are archived.
  Editing an archived project keeps it archived. Archiving a client archives its active projects (an earlier
  archive time stays); restoring a client leaves its projects archived.
- `list_items` gives `ProjectListItem { project, client, lastUsed, recordedMs30d }` from one grouped query:
  `lastUsed` is the latest recording start, `recordedMs30d` the recorded time in the 30 days before now (a
  running recording counts until now).
- `recent(limit)`: active projects by their latest recording start, newest first.
- `resolve(name_or_id)` (the CLI's `--project`, step 08): an exact id (archived too); else the active projects
  with exactly that name, ignoring case; else the active projects whose name starts with it. One match wins;
  none is `not_found`; several are `invalid_input` listing `name (id)`.
- Tags: `insert` of a taken name returns that tag; `list_with_usage` gives `TagListItem` (a `Tag` plus
  `recordings`, the number of recordings that carry it); `delete` removes it from every recording;
  `resolve_names` finds tags by name ignoring case, `not_found` naming the first missing one. Seven tags are
  seeded once per data store when it opens (`seed_defaults`).
- `projects/mod.rs` holds the Tauri commands; every write emits `data_changed { kind: projects | tags, day: null }`.

## Backup

`backup_export(path)` calls `backup::write_to`, which writes `Backup` as pretty JSON through `<path>.tmp` and a rename, and returns
`{ bytes }`. The path comes from the save panel (`choose_save_path`) and must be absolute.

```json
{
  "version": 1,
  "exportedAt": 1792540800000,
  "clients": [], "projects": [], "tags": [],
  "activities": [], "awaySpans": [],
  "recordings": [{ "id": "…", "projectId": "…", "start": 0, "end": 0, "note": "", "tagIds": [], "createdAt": 0, "updatedAt": 0 }],
  "rules": [], "savedReports": []
}
```

Rows have the shape of their IPC types (`Client`, `Project`, `Tag`, `ActivityRow`, `AwayRow`, `Recording`).
`rules` and `savedReports` stay empty until those features exist. There is no import yet.

## Migrations

Embedded at compile time with `sqlx::migrate!("./migrations")` and run by `Db::open`; `build.rs` rebuilds
the crate when the folder changes. Migrations are forward-only: add `NNNN_<name>.sql` with the next number and
never edit a file that has shipped. A downgrade restores a backup. `0002_drop_billing.sql` removed the billing
columns (`projects.billable`, `projects.hourly_rate`, `recordings.billable`): TimeTracker tracks time only.
`0003_client_colors.sql` gave clients the color most of their projects had and spread shades over each
client's projects.

## Tests

`test_support.rs` has `temp_db()` (a migrated file database in a temp dir, so WAL and migrations run as in the
app) and fixtures `activity(db, start, end, app, title)`, `project(db, name)`, `recording(db, project_id, start,
end)`. Repository tests use `Db::open_memory()` (one connection that never closes). See
[testing.md](testing.md#rust).
