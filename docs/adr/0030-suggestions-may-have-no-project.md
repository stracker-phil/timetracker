# 0030. A stored suggestion may have no project

## Status

Accepted (2026-10-27), implemented. Supersedes in part 0028 (a stored suggestion's project).

## Context

ADR 0028 made writes from other apps suggestions, each for a project. But a reminder or a model often
knows that time was spent, and on what, without knowing which project it belongs to. Making the caller
guess a project (or skip the write) loses the note; the owner would rather have it waiting in the Day view
and pick the project there.

## Decision

- **Project optional.** `stored_suggestions.project_id` may be NULL (migration
  `0012_suggestions_without_project.sql`). The CLI's `record` and the MCP `record` tool take the project
  only when they know it; a final write still needs one (`--final records time; add --project`).
- **Not approvable as it is.** Approving a suggestion without a project is refused ("The suggestion has no
  project yet; pick one first"), alone or among several. Adding the project is the owner's step: in the
  app's editor (which approves on save), or with `edit ID --project P` (stays a suggestion) or
  `edit ID --project P --final` (records it).
- **Shown in graphite.** The Day view shows such a suggestion like any stored one, in the graphite accent,
  with "No project" in place of the project's name. Its ✓ (and ↵) reads "Pick a project" and opens the
  editor. "Approve all" skips it and is off when nothing else is left to approve.
- A project, once set, can be changed but not cleared.

## Consequences

- Rule suggestions always have a project; only stored ones can lack it. `Suggestion.projectId` and
  `projectName` are optional for the page, as are the listing's `projectId` and `project` (CLI `--json`,
  MCP `suggestions`), and the CLI prints "no project" in its place.
- Backups keep the backup version: a stored suggestion's `projectId` may be `null`.
