# WP-0033: Console Integration

## Purpose

Expose independent inference operations through the AI Console prompt.

## Scope

`generate`, `complete`, and `explain`; streamed text and final metrics.

## Deliverables

- Command grammar entries and help text.
- Stream renderer integration.
- No chat mode or conversation state.

## Acceptance Criteria

Each command creates an independent runtime request.

## Dependencies

WP-0031, WP-0032.

## Future Work

Intent classification, planner explanations, and richer renderers.
