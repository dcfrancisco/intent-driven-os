# WP-0018: Runtime Event Bus

## Purpose

Provide a lightweight, decoupled path for runtime lifecycle and command events to reach console clients.

## Scope

In-process publish/subscribe delivery for runtime startup/shutdown, command lifecycle, thinking, streaming, warnings, errors, approval, input, and offline events. No network transport or durable event log.

## Deliverables

- Shared `EventBus` and subscription contract.
- `RuntimeEvent` taxonomy and ownership rules.
- Mock publisher behavior for the Phase 2 runtime.
- Console subscription and event draining strategy.
- Tests for fan-out, delivery, and stale subscriber removal.

## Acceptance Criteria

- Runtime publishers do not depend on console rendering types.
- Multiple subscribers can receive the same event.
- Disconnected subscribers do not prevent later events from being delivered.
- Event consumers can derive presence state without polling runtime internals.
- The event bus remains dependency-light and deterministic.

## Dependencies

WP-0002, WP-0009, WP-0016, WP-0020; ADR-0003, ADR-0004.

## Future Work

Define async, cross-process, replay, filtering, backpressure, and transport-backed event implementations.

