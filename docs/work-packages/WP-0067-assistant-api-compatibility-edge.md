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
  request without using `talvik` or OID.
- Responses identify the selected model/backend policy result and do not
  silently change provider/data-handling policy.
- Unsupported capabilities return structured errors rather than being faked.

## Dependencies

WP-0058 through WP-0062, WP-0066; ADR-0011, ADR-0013.

## Implementation status (2026-10-23)

An initial local OpenAI-compatible bridge is implemented at
`scripts/marina-openai-proxy.py`. It supports authenticated model discovery,
text chat/completions, and SSE streaming over loopback. It is intentionally
not the final Marina HTTP service: provider BYOK and quotas remain future work.
The native Talvik HTTP edge now accepts OpenAI-shaped tool definitions when the
loaded GGUF template advertises tool support, translates assistant tool calls
and tool results through the backend-neutral prompt boundary, and returns
normalized OpenAI `tool_calls`. The repeatable standard-client procedure is
`scripts/qualify-openai-client.py`; it covers model discovery, non-streaming,
SSE streaming, tool invocation, and a tool-result round trip. It requires the
optional `openai` Python package and a real GGUF daemon.

The implemented edge is a limited text Chat Completions subset, not full
OpenAI API conformance. Its current request/response behavior and known gaps
are documented in `docs/TALVIK-OPENAI-COMPATIBILITY.md`.

Final macOS x86_64 verification on 2026-10-23 used an isolated Python 3.11
environment and a fresh release daemon. The procedure returned
`OPENAI_CLIENT_ACCEPTED`; no IDE-specific client session was available. The
Windows and macOS arm64 procedures remain pending in the platform qualification
bundle.
