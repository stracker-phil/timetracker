# Architecture decision records

Each record (`NNNN-<slug>.md`, Nygard format: Status, Context, Decision, Consequences) explains why, in at
most a page; the docs pages explain how. An accepted decision is never edited: a new record supersedes it,
and a dated amendment under Status only corrects facts that changed.

| No. | Title | Status |
|---|---|---|
| [0001](0001-rust-owns-the-logic.md) | Rust owns the logic, React only renders | Accepted (2026-10-21), implemented |
| [0002](0002-ipc-in-api-files.md) | IPC only through `api.ts` files | Accepted (2026-10-21), implemented |
| [0003](0003-errors-as-kinds.md) | Errors cross IPC as `{kind, message, code}` | Accepted (2026-10-21), implemented |
| [0004](0004-generated-ipc-types.md) | TypeScript IPC types are generated from Rust with ts-rs | Accepted (2026-10-21), implemented |
| [0005](0005-activity-and-recordings-are-separate-layers.md) | Activity is immutable; recordings are a separate layer that shares only time | Accepted (2026-10-21), implemented |
| [0006](0006-sqlite-for-data-json-for-settings.md) | Activity and recordings live in SQLite (sqlx); settings in JSON files | Accepted (2026-10-21), implemented |
| [0007](0007-tracker-in-process-menu-bar-app.md) | The tracker runs inside the app process; the app keeps running without a window | Accepted (2026-10-21), implemented, amended |
| [0008](0008-capture-behind-a-source-trait.md) | macOS capture sits behind a `Source` trait; objc2 bindings live in one module | Accepted (2026-10-21), implemented, amended |
| [0009](0009-external-activity-with-a-source.md) | External activity enters through CLI, socket and deep link; every activity carries a `source` | Accepted (2026-10-21), implemented |
| [0010](0010-recordings-never-overlap-ops-with-undo.md) | Recordings never overlap; edits are Rust-side operations with inverses | Accepted (2026-10-21), implemented, superseded in part by 0020 |
| [0011](0011-utc-milliseconds-local-days.md) | Timestamps are UTC milliseconds; days are local calendar days computed in Rust | Accepted (2026-10-21), implemented |
| [0012](0012-timeline-layout-in-rust.md) | Timeline layout and timeline settings are pure Rust functions | Accepted (2026-10-21), implemented, amended |
| [0013](0013-no-network.md) | No network: no network code, CSP allows IPC only | Accepted (2026-10-21), implemented |
| [0014](0014-panic-unwind.md) | Keep `panic = "unwind"` | Accepted (2026-10-21), implemented, amended |
| [0015](0015-test-doubles-behind-test-util.md) | Test doubles ship behind a `test-util` feature | Accepted (2026-10-21), implemented, amended |
| [0016](0016-color-schemes.md) | Color schemes are complete token sets; no color literal outside a scheme | Accepted (2026-10-21), implemented |
| [0017](0017-cli-asks-the-running-app.md) | The CLI is a second process that asks the running app over the socket | Accepted (2026-10-21), implemented |
| [0018](0018-one-line-notes-tags-copyable-reports.md) | Notes are one line, tags classify work, reports are built for copying | Accepted (2026-10-21), implemented, amended, superseded in part by 0019 |
| [0019](0019-one-tag-per-recording.md) | A recording has at most one tag | Accepted (2026-10-21), implemented |
| [0020](0020-recordings-have-spans.md) | A recording holds one or more spans of time | Accepted (2026-10-22), implemented |
