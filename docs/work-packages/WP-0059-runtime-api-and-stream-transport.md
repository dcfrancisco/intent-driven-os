# WP-0059: Runtime API and Stream Transport

## Purpose

Expose the local runner as a versioned service so clients and future assistants
do not depend on in-process implementation details.

## Scope

Unix-socket resources for model list/inspect/load/unload, run/stream/stop,
health, hardware, and backend discovery; peer permissions, request idempotency,
disconnect behavior, and protocol version negotiation.

## Acceptance criteria

- `talvik` can perform the complete load-stream-cancel-unload flow through
  the service boundary.
- A disconnected client cannot stop or corrupt another request.
- API errors and stream terminal events are versioned and documented.
- No network listener is enabled by default.

## Dependencies

WP-RUNTIME-002, WP-0057; ADR-0004, ADR-0006, ADR-0011.

## Lifecycle closeout findings — 2026-10-17

- **MARINA-F-001 (P1, resolved):** the external acceptance probe omitted the
  required newline and used a timeout shorter than observed CPU first-token
  latency. Corrective harness: `scripts/accept-marina-lifecycle.py`.
- **MARINA-F-002 (P1, resolved):** Marina exposed terminal generation events
  before releasing the active-generation slot. The runtime now releases the
  slot before terminal delivery for completion, cancellation, and failure.
- **MARINA-F-003 (P2, open):** four console process-signal tests timed out
  before the startup marker in one workspace run. This is tracked under
  WP-0056 and is separate from the native Marina socket lifecycle gate.

Acceptance evidence: final release binaries passed two independent prompts,
streaming, real cancellation, post-cancel recovery, unload/reload,
post-reload inference, healthy status, and clean shutdown. MARINA-F-003 still
requires a stable process-test environment before that console gate closes.

## Implementation status (2026-10-07)

Implemented for the local first slice: `talvikd` and `talvik` communicate over
the user-scoped Unix socket on Unix and loopback TCP on Windows, with model
list/pull/load/unload, streaming generation, stable request IDs, and
request-scoped cancellation. Protocol version negotiation, framed transport,
named pipes, and crash/restart semantics remain open hardening work.
