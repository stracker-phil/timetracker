# 0004. TypeScript IPC types are generated from Rust with ts-rs

## Status

Accepted (2026-10-21, inherited from DataBee ADR 0014)

## Context

The page and the backend share dozens of types over IPC (day views, recordings, settings, reports). A
hand-written TypeScript mirror drifts: string unions the backend does not enforce, flags hidden in labels,
casts at use sites.

## Decision

Rust is the source of truth. `ts-rs` derives generate `src/api/types.gen.ts`; the output is committed, and
`bun run lint` fails when regenerating changes it. 64-bit integers map to `number` (`TS_RS_LARGE_INT` in
`.cargo/config.toml`); millisecond timestamps stay far below 2^53. Command signatures in `api` are still
hand-written.

## Consequences

- A shape change in Rust shows up as a TypeScript compile error.
- The build gains a codegen step (`bun run types`), and generated files appear in review diffs.
