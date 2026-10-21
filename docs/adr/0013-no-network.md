# 0013. No network: no network code, CSP allows IPC only

## Status

Accepted (2026-10-21), implemented.

## Context

The app records window titles, URLs and document paths. The PRD promises that no feature needs the network
and that there is no telemetry or update check. A promise in a settings page is weaker than one in the
dependency list.

## Decision

The crate has no HTTP client, no update plugin and no networking dependency; the Unix socket is the only
socket. `tauri.conf.json` sets `connect-src ipc: http://ipc.localhost` and `default-src 'self'`, and
`img-src 'self' data:`, so the webview cannot load remote content even if a window title contains a URL.
`tauri-plugin-opener` is allowed for "Reveal in Finder" and opening a URL the user clicks in a hover card,
which hands the URL to the system browser and loads nothing in-app. A unit test asserts that the macOS
dependency graph (`cargo tree --target aarch64-apple-darwin`) and `Cargo.toml` contain no `reqwest`, `hyper`,
`ureq` or `tauri-plugin-updater`.

## Consequences

- Updates are manual (a new build); the README says so.
- A later sync feature would need a new ADR superseding this one.
