# 0001. Rust owns the logic, React only renders

## Status

Accepted (2026-10-21), implemented. Inherited from DataBee ADR 0001.

## Context

A time tracker decides where a span starts and ends, how blocks pack into lanes, which gaps bridge, whether
two recordings overlap, how hours round and how a report reads. Each rule must be right once and testable
without a browser. The webview also renders arbitrary window titles and URLs, so it is the part most exposed
to surprises.

## Decision

Every rule lives in Rust: timeline layout, grouping and noise filtering, snapping, the overlap rule, rule
matching, report math and the text a report prints. The page asks for views and results, renders them and
forwards user intent (a selection, a drop, a field value); it never computes a recording's span or a report
on its own. Splitting logic between the two sides was rejected because the same rule would then exist twice,
or only in the half that cannot be unit tested without a DOM.

## Consequences

- The logic is unit tested in Rust and does not depend on page code being correct.
- More IPC types and round trips, even for small things such as snapping a dragged edge.
- A UI feature usually needs a Rust change first.
