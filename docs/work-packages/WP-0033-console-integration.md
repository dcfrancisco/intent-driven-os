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

## Talvik IPC generation

When Talvik is reachable, the console's adapter routes `:generate`,
`:explain`, and `:complete` through the daemon's local IPC stream instead of
the console's in-process runtime. The standalone `oid-marina-chat` client
requires IPC. The console retains its warned in-process fallback if the daemon
probe fails. IPC preserves client generation options, streams tokens and
terminal metrics, and forwards cancellation. OpenAI-compatible HTTP
qualification remains a separate client-integration path.
