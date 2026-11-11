# 0047. A major version needs the owner's approval

## Status

Accepted (2026-11-11), implemented. Supersedes in part 0043 (the "major" bump).

## Context

ADR 0043 made a changed or removed command, flag, output or feature a major bump. TimeTracker is a personal
tool with one user, who also writes the scripts that use it; a refused `timetracker approve` of a running
suggested timer (ADR 0046) was released as 2.0.0, which the owner did not want.

## Decision

- **Changes that break existing use are minor**, like new features and visible behavior.
- **A major bump needs the owner's explicit approval**, asked before releasing; never chosen from the log alone.
- Patch stays as it was: fixes, polish, performance and docs only.
- Rejected: keeping semver's strict meaning (it signals breakage to other consumers, which this tool does not
  have).

## Consequences

- The 2.0.0 release was taken back before it was pushed (its commit and tag removed) and released as 1.3.0.
