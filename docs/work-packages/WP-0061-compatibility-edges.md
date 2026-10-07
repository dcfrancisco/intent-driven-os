# WP-0061: Compatibility Edges

## Purpose

Make OID easy to adopt from Ollama-like and OpenAI-compatible clients without
turning either external protocol into the internal architecture.

## Scope

Explicitly supported subset of model list/show, generate/chat streaming,
stop/cancel, health, errors, and capability discovery. Document unsupported
semantics and preserve OID request IDs, policy, routing, and audit metadata.

## Acceptance criteria

- Golden fixtures cover request/response and streaming translation.
- Provider-native options are rejected or mapped explicitly, never ignored.
- Compatibility endpoints cannot bypass local policy, admission, or audit.
- Remote credentials, endpoint health, and cost attribution are isolated from
  local model state.

## Dependencies

WP-0059, WP-0060; ADR-0003, ADR-0006, ADR-0011.
