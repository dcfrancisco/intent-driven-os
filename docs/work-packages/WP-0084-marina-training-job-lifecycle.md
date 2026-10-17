# WP-0084: Marina Training Job Lifecycle and Isolation

## Purpose

Implement the governed job lifecycle independently from the production
inference worker.

## Scope

- Job persistence, validation, queueing, cancellation, retries, checkpoints,
  recovery, terminal states, and evidence.
- Worker process isolation, resource limits, disk checks, concurrency limits,
  and inference priority.
- Authorization and approval gates for sensitive jobs.

## Acceptance criteria

- `PENDING -> VALIDATING -> QUEUED -> RUNNING -> EVALUATING -> COMPLETED`
  and failure/cancellation transitions are deterministic.
- Failed/interrupted jobs cannot alter the active inference model.
- Cancellation and restart recovery do not create duplicate terminal records or
  uncontrolled restart loops.
- Training workers cannot obtain unrestricted OID or host authority.

## Dependencies

WP-0082, WP-0083, ADR-0020, WP-0077, WP-0071.

## Status

Planned. Lifecycle implementation is separate from inference serving.
