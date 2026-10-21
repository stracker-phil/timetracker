# 0005. Activity is immutable; recordings are a separate layer that shares only time

## Status

Accepted (2026-10-21)

## Context

Timing assigns activity to projects and builds entries on top; Timely keeps a private activity track and
lets the user build entries over it without linking the two. The PRD picks Timely's model: raw data stays
raw and never appears in reports, and a recording must stay valid when activity is deleted for privacy.

## Decision

`activities` and `away_spans` are written only by the tracker and external sources, never updated (except
closing an open span) and deleted only by an explicit user action. `recordings` carry no foreign key to
activity. Creating a recording from a selection copies the selected time span; whether activity is "covered"
is computed at read time by time overlap. Rules produce suggestions, which are recordings only once
accepted. Linking recordings to activity ids was rejected: it breaks on activity deletion, and it tempts
reports into showing raw data.

## Consequences

- Deleting a day of activity leaves every recording intact.
- "Covered" and "tracked vs recorded" totals are computed per view, cheaply, from time ranges.
- A recording cannot show "which windows it came from" later; the note is the user's record.
