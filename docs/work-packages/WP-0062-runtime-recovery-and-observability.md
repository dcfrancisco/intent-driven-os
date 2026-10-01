# WP-0062: Runtime Recovery and Observability

## Purpose

Make failures in model loading, streaming, routing, and backend processes
recoverable and diagnosable.

## Scope

Backend supervision, timeouts, crash classification, retry/fallback limits,
request metrics, routing explanations, append-only audit records, and restart
reconciliation.

## Acceptance criteria

- Backend crash, client disconnect, cancellation, timeout, and partial stream
  each have deterministic terminal states.
- Restart cannot report a model as loaded when its handle is gone.
- Metrics and audit records correlate model, request, backend, policy, and
  route decision IDs.
- Recovery torture tests run on actual Linux as well as harness tests.

## Dependencies

WP-0043, WP-0046, WP-0049, WP-0056, WP-0060; ADR-0006, ADR-0009, ADR-0011.
