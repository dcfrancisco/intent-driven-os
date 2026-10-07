# WP-0067: Assistant API Compatibility Edge

## Purpose

Provide a stable API for AI assistants and applications while keeping Marina
policy, routing, and backend boundaries authoritative.

## Scope

- Versioned assistant request/response contracts.
- OpenAI-compatible chat/completions translation where capability-compatible.
- Streaming events, usage metrics, model identity, cancellation, and errors.
- Explicit tool-call and structured-output capability negotiation.
- Authentication, quotas, audit correlation, and local-only defaults.

## Acceptance criteria

- An external assistant can discover health/models and complete a streaming
  request without using `marinactl` or OID.
- Responses identify the selected model/backend policy result and do not
  silently change provider/data-handling policy.
- Unsupported capabilities return structured errors rather than being faked.

## Dependencies

WP-0058 through WP-0062, WP-0066; ADR-0011, ADR-0013.

## Implementation status (2026-10-07)

An initial local OpenAI-compatible bridge is implemented at
`scripts/marina-openai-proxy.py`. It supports authenticated model discovery,
text chat/completions, and SSE streaming over loopback. It is intentionally
not the final Marina HTTP service: provider BYOK, scopes/quotas, structured
tool calls, and Windows HTTP parity remain future work.
