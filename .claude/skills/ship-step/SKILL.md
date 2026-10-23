---
name: ship-step
description: Finish a development step in TimeTracker - clean up and review the code, run tests and lint through the ci agent, build and launch the app, then commit. Use at the end of every feature or change.
---

1. Run the `clean-up-code` skill on the uncommitted changes.
2. Spawn one `review-code-cleanliness` agent per changed source file (`review CHANGES in <file>`), in
   parallel. Apply the findings that hold; weigh the `Consider` questions and mention open ones in the report.
3. Run `cargo fmt --manifest-path src-tauri/Cargo.toml`, then spawn one `ci` agent for "All unit tests", then
   "Lint" (one after the other: both regenerate `src/api/types.gen.ts`). If the change touches a module with `#[ignore]` integration tests, also run
   "Integration". Fix failures (test fixes go back to the `write-*-unit-test` agent) and run `ci` again until
   green.
4. `bun run build:app`, then `scripts/dev/launch.sh [CLI args]`. This restarts the app on the user's terminal
   screen without taking focus; never use `bun run app` or `open` (they steal focus on the user's other screen).
   The debug app is never installed: call its CLI by path
   (`src-tauri/target/debug/bundle/macos/TimeTracker (Debug).app/Contents/MacOS/timetracker`), not `timetracker`.
   Drive the UI with `scripts/dev/js.sh '<js>'` (fire-and-forget; wrap code in an IIFE, the store is
   `window.timetracker`) and check it with `scripts/dev/screenshot.sh <png>`. Never use AppleScript clicks or
   keystrokes; they need focus.
5. Update the affected `docs/` page if behavior, commands, files or extension points changed.
6. Stage the step's files by path (check `git status` for stray files), then commit following
   `.claude/rules/commits.md`. Never push.
7. `bun run release` (in the background; it takes minutes): `scripts/install.sh` installs the release build
   as `~/Applications/TimeTracker.app` and `timetracker`, and restarts the running app on it, so the user's
   app always matches the last commit.
8. Report to the user: what changed, the `ci` summary, open review questions, and that the release app was
   installed.
