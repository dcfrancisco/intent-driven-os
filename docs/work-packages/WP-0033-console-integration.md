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

The console's local mode routes `:generate`, `:explain`, and `:complete`
through the daemon's versioned local IPC stream; it requires a compatible
Talvik daemon and never falls back to in-process inference. The standalone
`oid-marina-chat` client also requires IPC. IPC preserves client generation
options, streams tokens and terminal metrics, and forwards cancellation.
`oid-console` rejects HTTP backend selection: it is the local Linux-first
MarinaOS console, while Talvik HTTP/OpenAI-compatible routes are reserved for
external clients.
