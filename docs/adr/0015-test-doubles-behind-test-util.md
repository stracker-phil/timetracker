# 0015. Test doubles ship behind a `test-util` feature

## Status

Accepted (2026-10-21, inherited from DataBee ADR 0019)

## Context

Unit tests need a fake capture source and a temp SQLite database; integration tests in `src-tauri/tests/`
are a separate crate and cannot see `#[cfg(test)]` items. Compiling the doubles unconditionally would put
dead code into the release binary.

## Decision

Doubles (`capture::fake::FakeSource`, `test_support::temp_db`, fixture builders) compile under
`cfg(any(test, feature = "test-util"))`. The crate lists itself as a dev-dependency with the `test-util`
feature, so integration tests get the doubles while release builds never include them.

## Consequences

- Doubles are available wherever tests run and absent from the app.
- Every new double uses the same `cfg` gate.
- The self dev-dependency is an unusual Cargo pattern; `Cargo.toml` comments point here.
