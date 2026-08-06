# WP-0035: Cancellation

## Purpose

Stop an individual generation safely.

## Scope

Atomic cancellation signal, backend callback checks, cancellation event, and
preservation of runtime/model process state.

## Deliverables

- Stream cancellation handle.
- Native decode-loop cancellation checks.

## Acceptance Criteria

Cancellation does not terminate the console or unload the active model.

## Dependencies

WP-0032, WP-0029.

## Future Work

Hard cancellation, timeout policy, and cooperative cancellation across remote
providers.
