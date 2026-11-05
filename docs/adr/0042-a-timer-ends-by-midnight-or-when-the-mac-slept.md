# 0042. A timer ends by midnight, or where the Mac slept

## Status

Accepted (2026-11-05), implemented.

## Context

A timer runs until it is stopped. One forgotten in the evening ran through the night or the weekend, and the
owner had to find the recording and cut it back by hand; one recording then also spanned two days, which the
Day view, the reports and the open-start check (a timer can't start before today's midnight) all treat as an
exception. The Mac knows when it went to sleep: the tracker writes a `sleep` away span at the system's sleep
notification.

The running timer's recording also became editable while it runs (its closed spans and its open start save
at once, Save keeps it running), which leaves only its end to the timer.

## Decision

- **A timer never passes a calendar day.** Every stop ends it by the local midnight after its open start
  (`recordings::auto_stop::capped_stop_time`): `stop_timer`, and the stop that starting another timer makes;
  that new timer then starts at the free start of now, not at midnight. A recording or a stored suggestion
  continues only when it lies wholly on the new start's day.
- **A forgotten timer stops on its own** (`auto_stop_in`, pure, the time zone a parameter): at the start of
  the first sleep from its open start on that lasts more than 60 minutes, and in any case at midnight, or
  at the start of a sleep across midnight. Timer started 14:00, asleep from 17:00 to 18:05: at 18:05 it ends
  at 17:00. Asleep from 23:15: at midnight it ends at 23:15. Both kinds of timer, a recording and a suggested
  one, stop alike, each as one undoable change ("Stop timer at midnight", "Stop timer (the Mac slept)"); the
  end is snapped like any stop.
- **Checked on the minute tick, at startup and on wake.** `timers::start_ticking` checks before each publish
  (its first tick is the app's start); `timers::auto_stop::watch_wake` checks when the tracker closes a sleep
  span. A gap of more than an hour between two ticks counts as a sleep from the earlier tick, for a sleep the
  tracker did not record (paused, or without permission).
- **The running timer's recording is edited like any other** while it runs: its closed spans, project, note
  and tag change, and its open span's start moves (not before today's midnight); only its end is live, and
  it moves only when the timer stops.
- Rejected: continuing at midnight into a new recording on the next day (a forgotten timer would still count
  the night, now split in two); stopping on idle or a locked screen (reading, a call or a meeting away from
  the keyboard is work the timer is meant to keep); no automatic stop (the forgotten night stays the owner's
  chore).

## Consequences

- A recording made by a timer lies on one day, so the timeline, the totals and the reports never split a
  timer's span.
- While the app is quit nothing records a sleep, so a timer left running then stops only at its midnight,
  the next time the app runs.
- A sleep under an hour leaves the timer running across it, as before.
