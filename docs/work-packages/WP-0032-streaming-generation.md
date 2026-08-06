# WP-0032: Streaming Generation

## Purpose

Deliver token output incrementally while keeping backend ownership isolated.

## Scope

Worker-backed streams, token callbacks, event publication, and cancellation
signals.

## Deliverables

- Non-blocking runtime stream handle.
- Token and completion messages.
- UI subscription path.

## Acceptance Criteria

Clients receive output fragments before completion and can cancel a request
without terminating the runtime.

## Dependencies

WP-0031, WP-0018, WP-0017.

## Future Work

Async transports, multiplexing, backpressure, and remote streams.
