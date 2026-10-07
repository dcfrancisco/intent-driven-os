# WP-0059: Runtime API and Stream Transport

## Purpose

Expose the local runner as a versioned service so clients and future assistants
do not depend on in-process implementation details.

## Scope

Unix-socket resources for model list/inspect/load/unload, run/stream/stop,
health, hardware, and backend discovery; peer permissions, request idempotency,
disconnect behavior, and protocol version negotiation.

## Acceptance criteria

- `marinactl` can perform the complete load-stream-cancel-unload flow through
  the service boundary.
- A disconnected client cannot stop or corrupt another request.
- API errors and stream terminal events are versioned and documented.
- No network listener is enabled by default.

## Dependencies

WP-RUNTIME-002, WP-0057; ADR-0004, ADR-0006, ADR-0011.

## Implementation status (2026-10-07)

Implemented for the local first slice: `marina` and `marinactl` communicate over
the user-scoped Unix socket on Unix and loopback TCP on Windows, with model
list/pull/load/unload, streaming generation, stable request IDs, and
request-scoped cancellation. Protocol version negotiation, framed transport,
named pipes, and crash/restart semantics remain open hardening work.
