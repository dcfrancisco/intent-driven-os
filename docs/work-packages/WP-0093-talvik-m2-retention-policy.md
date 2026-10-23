# WP-0093: Talvik M2 Retention and Resource Policy

## Purpose

Make idle retention and memory-pressure behavior explicit and observable while
preserving the one-active-generation safety policy.

## Scope

- Define keep-alive/idle-retention semantics for local and compatibility edges.
- Record eviction and reload events without sensitive request content.
- Verify active-request protection, pinning, bounded recovery, and restart
  reconciliation with deterministic fixtures and native GGUF evidence.

## Dependencies

WP-0058, WP-0062, WP-0091; ADR-0007, ADR-0025.

## Acceptance criteria

- Configured retention is visible in diagnostics and does not silently reduce
  requested context size.
- Idle unpinned models may be evicted only within a bounded recovery budget.
- Busy and pinned models are never evicted.
- Rejected admission leaves existing residents and registry state healthy.

## Status

Implementation slice verified on `feature/talvik-m2-runtime-ide`.

Evidence: `runtime/src/service.rs` now separates incremental physical-memory
admission from aggregate configured-budget accounting, preserves resident
models during normal generation admission, serializes lifecycle admission and
native load/unload transitions, protects pinned/active models, and uses
bounded LRU recovery for load/reload. Deterministic runtime tests cover
the exact memory boundary, resident-model accounting, pinning, eviction,
reload, and queued cancellation. Native process-signal qualification also
passes in parallel after the harness was isolated from shared user state.

Idle retention remains an on-demand scheduler policy: expired idle models are
eligible for bounded pressure recovery; no background eviction thread is
introduced. Per-request retention control and native two-model qualification
remain open acceptance work.
