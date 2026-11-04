# Demo data only

The repo is published as source. Nothing in it may carry the owner's real data.

- Code, comments, docs, ADRs, tests, fixtures, seeds, commit messages and screenshots use made-up values
  only: invented projects, clients, tags, notes, apps' window titles, document paths, URLs and dates. Use
  `example.com` addresses and hosts. Never copy a real name, customer, project, window title, path under the
  owner's home or browser address into the repo, not even as an example.
- The debug build never sees the owner's data: it runs on its own data folder, captures made-up windows
  (`capture/demo.rs`) and starts from the showcase in `src-tauri/src/test_support/showcase/` the first time
  it runs ([ADR 0031](../../docs/adr/0031-the-debug-build-runs-on-demo-data.md)). Real capture
  (`TIMETRACKER_REAL_CAPTURE=1`) is for checking capture code only; nothing it records leaves the machine.
- Screenshots that are committed (`docs/screens/`) come from the debug app on the showcase, produced by a
  script (`scripts/dev/screenshot-readme.sh`), never from the release app.
- When the owner shares real examples to explain a feature, translate them into neutral demo values before
  they reach a file.
