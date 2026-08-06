# WP-0036: Generation Events

## Purpose

Make inference lifecycle observable to the console and future services.

## Scope

Started, first-token, token-generated, completed, cancelled, and failed events.

## Deliverables

- Shared event variants.
- Presence/status integration.
- Event publication tests.

## Acceptance Criteria

Every admitted generation reaches a terminal event unless the event consumer
disconnects; failures are represented separately from cancellation.

## Dependencies

WP-0018, WP-0017, WP-0031, WP-0032.

## Future Work

Structured event payloads, tracing correlation, and transport serialization.
