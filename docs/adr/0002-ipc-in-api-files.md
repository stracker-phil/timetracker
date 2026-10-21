# 0002. IPC only through `api.ts` files

## Status

Accepted (2026-10-21), implemented. Inherited from DataBee ADR 0002.

## Context

UI tests need to replace the backend. `invoke` and `listen` calls scattered over components make that
impossible without mocking Tauri itself, and they hide the contract between page and backend. Secondary
windows (the away prompt, the timer picker) share that contract.

## Decision

Only API files import `@tauri-apps/api` or a `@tauri-apps/plugin-*` package: the core's `src/api/index.ts`,
and one `src/<module>/api.ts` per optional module with its own folder. Everything else imports the `api`
object; secondary windows import the same file. A boundary test fails on any other file that imports Tauri.
Tests mock exactly that module with `createApiMock()`.

## Consequences

- Tests mock one module per backend; the contract is visible in one place.
- Every command needs a thin hand-written wrapper.
- Event subscriptions (`api.onTrackerStatus`, …) live in the same file, so tests can drive them.
