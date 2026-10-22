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
`Db::open_migrating` does the same and also returns what a migration dropped (`db::one_tag::TagsDropped`,
see [Migrations](#migrations)); `DataStore::open` uses it and keeps the message for the launch toast.

`Db::open_read_only(path)` is for `timetracker mcp` ([backend.md](backend.md#mcp-server)), a second process
beside the app: `read_only(true)`, `create_if_missing(false)`, a 2 s busy timeout, 2 connections, and the
journal mode left as the app set it. WAL readers never block the writer and see each commit on their next
query. Nothing is migrated or seeded: a missing file is `not_found` "TimeTracker has no data yet (… not
found)", a file whose newest applied migration is older than this build's is refused with "Open TimeTracker
once to update its data", and a newer one is read as it is.

`AppState.data` is a `DataStore`: the file path plus the open `Db` or the reason it could not be opened, each
behind an `RwLock` so the folder can move (below). `AppState::open` opens it before Tauri starts
(`tauri::async_runtime::block_on`). When opening fails, the app logs it and runs without a database:
`DataStore::db()` returns `db` "The data store is not available", and Settings → Data shows the error. On
`RunEvent::Exit` the pool closes.

A `dataDir` that is not a folder at startup (an unmounted volume) is left in the settings; `AppState::open`
(`data_dir_at_startup`) logs an error, uses `db::default_data_dir` for this run and queues a launch error,
which the page toasts on load: "Data folder … not found; using the default".

### Moving the data folder

`db::relocate::move_data(data, settings, tracker, target)`, behind the `data_move` command (Settings → Data →
Move…). `DataStore`'s own mutex makes moves run one at a time. In order:

1. `prepare_target` refuses (`invalid_input`) a relative path, a path inside the current data folder (checked
   before and after resolving symlinks), something that is not a folder, a folder that already holds
   `timetracker.sqlite`, and one it cannot write to (a probe file). A missing folder is created, mode 0700; an
   existing one keeps its mode.
2. `Tracker::suspend` takes the tracker's state lock, so ticks, events, pause and resume wait; no away span is
   written and the open activity span stays open. `DataStore::take` makes the store unavailable ("The data
   folder is moving"): commands fail with `db`, and the timer tick reads nothing (it writes nothing anyway;
   `data_move` publishes the timer when done).
3. `PRAGMA wal_checkpoint(TRUNCATE)`, then the pool closes.
4. `timetracker.sqlite`, and `-wal`/`-shm` when present, are copied; each copy must match its source's size.
   `Db::open_read_only` on the copy (which also checks the schema) and `PRAGMA integrity_check` must say `ok`.
5. `SettingsStore::set_data_dir` writes the resolved folder to `dataDir`.
6. `Db::open` on the copy goes into the store and into the tracker (`Suspended::resume`), which records on
   from where it stopped: the copy holds the same rows and ids.

A failure in steps 3–6 deletes the copies, reopens the old file, resumes the tracker on it and returns the
error; when the copy fails to open after step 5, `dataDir` goes back to its previous value first. The old files
always stay; the user deletes them in Finder. `update_settings` keeps the stored `dataDir` whatever the page
sends, so a page holding settings from before a move never moves the data back.

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
| `recordings` | time confirmed for a project: `id` (UUID), `project_id`, `note`, `tag_id` (NULL without a tag; deleting the tag sets it NULL), `created_at`, `updated_at`; the time is in its spans |
| `recording_spans` | the time of a recording ([ADR 0020](adr/0020-recordings-have-spans.md)): `id` (UUID), `recording_id` (deleting the recording deletes them), `start`, `end` (NULL only for the running timer's span); indexed on `start` and `(recording_id, start)` |
| `saved_reports` | `id`, `name`, `filter` (a `ReportFilter` as JSON), `position` |
| `rules` | `id`, `name`, `project_id`, `position` (the lowest wins), `enabled`, `note`, `tag_id` (as for recordings) |
| `rule_conditions` | `id`, `rule_id` (deleting the rule deletes them), `position`, `field`, `op`, `value` (the serde names of `RuleField` and `RuleOp`) |

`end` is a SQL keyword, so queries quote it (`"end"`). `activities` and `away_spans` use AUTOINCREMENT so a
deleted id is never reused and an undo can restore rows by id.

### Data rules

- Activity and recordings are separate layers ([ADR 0005](adr/0005-activity-and-recordings-are-separate-layers.md)):
  nothing turns activity into recordings on its own.
- Activity and away rows are never edited, except to set `end` on the open span. Deleting activity is an
  explicit user action (`activity_delete`, `timetracker activity delete`) and deletes crossing rows whole:
  every activity and away row with `start < to AND (end IS NULL OR end > from)`, in one transaction, with
  recordings untouched. For 30 s the deleted rows stay in memory; `activity_delete_undo` re-inserts them
  under their ids (open ones closed at the time of the delete). The flow is in
  [backend.md](backend.md#activity-deletion).
- Spans are stored unsplit. A reader clips them to the range it shows, so a span across midnight shows on
  both days.
- A recording has at least one span; its spans never overlap each other (touching is fine), and no span shares
  time with a span of another recording ([ADR 0020](adr/0020-recordings-have-spans.md)). A recording's
  duration is the sum of its spans; it belongs to every day one of its spans touches, and a day counts only
  the parts of the spans inside it. The running timer is the one recording with an open span (`end` NULL,
  always its last), which counts as overlapping everything after its start. Stopping it sets the span's
  `end` to `stop_time`: the snapped now, at least a minute after the span's start. The recordings feature
  enforces this; the repository only checks a recording's own spans and offers the `overlapping` query.
- `recordings` keeps no cached start or end: readers that sort by time use the first span
  (`min(recording_spans.start)`), so there is one source of truth for the time.
- A note is one line ([ADR 0018](adr/0018-one-line-notes-tags-copyable-reports.md)).
- A recording or rule has at most one tag ([ADR 0019](adr/0019-one-tag-per-recording.md)).

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
| `tags.rs` | `list`, `list_with_usage`, `insert(name)`, `rename(id, name)`, `delete(id)`, `resolve_name(name)`, `resolve_optional_name(name?)` (the id, or `None` without a name), `seed_defaults` | names unique ignoring case: `insert` of a taken name returns the existing tag, `rename` to it is `invalid_input`; `DEFAULT_TAGS` are seeded once per data store (`meta.tags_seeded`), so deleting them all does not bring them back |
| `activities.rs` | `apps_seen(from, now)`, `list_clipped(from, to, now)`, `close_stale(source, duration)`, `replace_open(id, NewActivity)`, `insert(NewActivity) -> id`, `close_open(source, end)`, `open_by_source`, `overlapping(source, start, end)`, `sources_with_prefix(prefix)` (latest first); `insert_on`, `get_on`, `close_open_on`, `open_by_source_on`, `overlapping_on` take a connection so a caller can combine them in a transaction; `list_for_range(from, to)`, `list_sources(from, to)`, `delete_range(from, to)`, `count_range(from, to)`, `count`, `first_start`; `take_range_on(from, to)` deletes and returns the rows, `restore_on(row)` inserts one under its id | ranges are `[from, to)` and include open spans; touching edges do not overlap; results are not clipped; `close_open` never sets an end before the start |
| `away.rs` | `insert(NewAway) -> id`, `close_open(end)`, `open`, `list_for_range`, `delete_range`, `count_range`, `take_range_on`, `restore_on` | as for activities; `AwayReason` is stored and sent as `idle`, `sleep`, `lock`, `paused` |
| `recordings.rs` | `insert(NewRecording)`, `get`, `update(id, RecordingPatch)`, `delete(id) -> Recording`, `list_for_range`, `running`, `latest(limit)`, `overlapping(start, end, except)`, `count`; `put_on(recording)` writes a row with all its spans, `remove_on`, `get_on`, `overlapping_on` take a connection; `check_spans`, `respanned` | one transaction per write; the project and the tag (`tag_id`, optional) must exist (`not_found`); a note with a line break is `invalid_input`, notes are stored trimmed; a recording needs a span, each ending after it starts, none overlapping another of the recording, only the last open; a query returns whole recordings with every span (also those outside the range) by first span; `delete` returns the row so it can be restored under its id |
| `backup.rs` | `export(db) -> Backup`, `write_to(db, path)` | every table, read in one transaction |
| `one_tag.rs` | `back_up(migrator, pool, dir) -> Option<TagsDropped>`, `TagsDropped::message` | see [Migrations](#migrations) |

`Recording` carries `spans: RecordingSpan[]` (`{ id, start, end }`, by start); `start()`, `end()`,
`duration(now)`, `is_running()`, `open_span()` and `span(id)` read them. `RecordingPatch` leaves absent fields
as they are; `spans` (closed `Span`s) replaces every span, the n-th by start keeping the id of the n-th current
one (`respanned`); `tagId: null` removes the tag (`Option<Option<…>>`).

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
  `recordings`, the number of recordings that carry it); `delete` removes it from every recording and rule;
  `resolve_name` finds a tag by name ignoring case, `not_found` naming it. Seven tags are seeded once per
  data store when it opens (`seed_defaults`).
- `projects/mod.rs` holds the Tauri commands; every write emits `data_changed { kind: projects | tags, day: null }`.

## Backup

`backup_export(path)` calls `backup::write_to`, which writes `Backup` as pretty JSON through `<path>.tmp` and a rename, and returns
`{ bytes, recordings, activities }`. The path comes from the save panel (`choose_save_path`) and must be absolute.

```json
{
  "version": 3,
  "exportedAt": 1792540800000,
  "clients": [], "projects": [], "tags": [],
  "activities": [], "awaySpans": [],
  "recordings": [{ "id": "…", "projectId": "…", "note": "", "tagId": null, "createdAt": 0, "updatedAt": 0, "spans": [{ "id": "…", "start": 0, "end": 0 }] }],
  "rules": [{ "id": "…", "name": "Slack → Acme", "projectId": "…", "position": 0, "enabled": true, "note": "", "tagId": null, "conditions": [{ "id": "…", "field": "app", "op": "is", "value": "Slack" }] }], "savedReports": [{ "id": "…", "name": "This month", "filter": { "range": { "kind": "thisMonth" }, "…": "…" }, "position": 0 }]
}
```

Rows have the shape of their IPC types (`Client`, `Project`, `Tag`, `ActivityRow`, `AwayRow`, `Recording`,
`Rule`, `SavedReport`). Version 1 (before migration 0006) had `tagIds` arrays on recordings and rules
instead of `tagId`; the pre-0006 backup is written in it. Version 2 (before migration 0007) had `start` and
`end` on recordings instead of `spans`. There is no import yet; one would map a version 1
row to its first tag by name.

## Migrations

Embedded at compile time with `sqlx::migrate!("./migrations")` and run by `Db::open`; `build.rs` rebuilds
the crate when the folder changes. Migrations are forward-only: add `NNNN_<name>.sql` with the next number and
never edit a file that has shipped. A downgrade restores a backup. `0002_drop_billing.sql` removed the billing
columns (`projects.billable`, `projects.hourly_rate`, `recordings.billable`): TimeTracker tracks time only.
`0003_client_colors.sql` gave clients the color most of their projects had and spread shades over each
client's projects. `0004_saved_reports.sql` adds `saved_reports (id, name, filter, position)`; `0005_rules.sql` adds `rules` and
`rule_conditions`.

`0006_one_tag.sql` ([ADR 0019](adr/0019-one-tag-per-recording.md)) adds `recordings.tag_id` and `rules.tag_id`,
fills each with the first of the row's tags by name (ignoring case: the order Settings shows), and drops
`recording_tags` and `rules.tag_ids`. Before it, `Db::init` (for a file, not in memory) calls
`one_tag::back_up`: it runs the migrations up to 0005 (`Migrator::run_to`); when `recording_tags` still exists
and a recording or a rule has more than one existing tag, it writes a version 1 backup (`tagIds` arrays) to
`pre-0006-<YYYY-MM-DD>.json` in the data folder, through a temp file. A failed backup fails the open, so
no tag is dropped without one. `TagsDropped { recordings, rules, backup }` comes back through
`Db::open_migrating`; `AppState::open` queues its `message()` as a `LaunchEvent::Notice`, which the page shows
as an info toast: "One tag per recording: 3 recordings lost tags (each kept its first by name). Backup: …".

`0007_recording_spans.sql` ([ADR 0020](adr/0020-recordings-have-spans.md)) adds `recording_spans`, gives every
recording one span with its `start` and `end` under the recording's own id, and drops `recordings.start`,
`recordings.end` and the `recordings_start` index. Nothing is lost, so it needs no backup.

## Tests

`test_support/` has `temp_db()` (a migrated file database in a temp dir, so WAL and migrations run as in the
app) and fixtures `activity(db, start, end, app, title)`, `project(db, name)`, `recording(db, project_id, start,
end)` (one span). Repository tests use `Db::open_memory()` (one connection that never closes). See
[testing.md](testing.md#rust).

### Seed fixture and performance

`test_support/seed.rs` (`test-util` and debug builds) writes a deterministic data set: `seed_days(db, first,
last, seed)` fills every workday of the range, `seed_years(db, years, seed)` the `years` up to yesterday.
Per workday, 08:00–18:00 local: `ACTIVITIES_PER_DAY` (500) back-to-back `tracker` spans over 12 apps and 40
titles, `AWAY_PER_DAY` (3) away spans (two short `idle`, a `lock` lunch; activity ends where they start) and
`RECORDINGS_PER_DAY` (6) non-overlapping recordings over 5 projects of 2 clients, with notes and none or one
of 5 tags. Clients, projects and tags are reused by name. A small splitmix64 generator keeps it free of `rand`;
the same seed writes the same rows except the random recording ids. Each day is one multi-row transaction:
two years (522 workdays, 261 000 activity spans, 3 132 recordings) seed in about 1.5 s in a debug build,
0.7 s in release.

`tests/performance.rs` (`#[ignore]`, so `test:db`) seeds two years into a temp file and asserts medians of
20 runs for a workday a year back:

| Path | Limit | Measured (debug / release) |
|---|---|---|
| `day::commands::load` (Day view with suggestions) | 50 ms | 14 / 12 ms |
| `reports::run` for that month | 200 ms | 3 / 2 ms |
| `calendar::commands::load_week` | 100 ms | 25 / 17 ms |
| `calendar::commands::load_month` | 150 ms | 2 / 2 ms |
| `day::list::list` of `day::commands::view_of` (List mode) | 50 ms | 13 / 10 ms |

The schema's indexes suffice. The debug profile compiles `libsqlite3-sys` with `opt-level = 3`
(`Cargo.toml`): unoptimized SQLite made the week view take 120 ms in debug builds.
