# 0019. A recording has at most one tag

## Status

Accepted (2026-10-21), implemented. Supersedes the tag part of
[0018](0018-one-line-notes-tags-copyable-reports.md).

## Context

ADR 0018 made tags many-to-many: a recording could carry any number. In use, the owner gives each recording
one kind of work, as in Timely, and several tags only made reports harder to read: grouped by tag, a
recording with two tags counted in two groups, so the groups added up to more than the total and the report
needed a caveat. The editor's multi-select picker was also slower than picking one value.

## Decision

A recording has zero or one tag: `recordings.tag_id`, a nullable reference to `tags` that becomes `NULL`
when the tag is deleted. Rules carry one tag the same way (`rules.tag_id`). Every input and output type has
`tagId: string | null` instead of `tagIds`. The CLI takes one `--tag` for `record` and `start`; MCP
recordings expose `tag` (a name or `null`). Grouped by tag, a report counts each recording once, with
"Untagged" for recordings without one, so the groups always add up to the total.

Migration `0006_one_tag.sql` keeps, of several tags, the first by name (the order Settings shows). Before it
runs on data where a recording or rule has more than one tag, the app writes a version 1 backup to
`pre-0006-<date>.json` in the data folder and the launch toast says how many rows lost tags and where the
backup is. The backup format moves to version 2.

## Consequences

- Reports by tag are a partition of the time; the `tagOverlap` flag and its caveat are gone.
- The tag picker is a single select with "No tag".
- Recordings that had several tags lose all but one; the pre-0006 backup keeps the rest.
- A second classification later is a new field, not more tags.
