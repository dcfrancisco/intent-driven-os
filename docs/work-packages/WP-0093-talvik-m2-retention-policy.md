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

Planned; no concurrent-generation work is authorized by this package.
