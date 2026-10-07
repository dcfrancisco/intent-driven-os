# WP-0070: AI Capability and Provider Contract

## Purpose

Define the canonical AI service contract beneath Ollama, OpenAI, Anthropic,
MCP, and future A2A compatibility edges.

## Scope

- Versioned model manifest and provider/backend capability schemas.
- Chat sessions, messages, typed content parts, generation controls, usage,
  cancellation, keep-alive, and structured errors.
- Tool-call proposals, structured output, embeddings, multimodal inputs, and
  explicit capability negotiation.
- Provider credentials, local/offline policy, model aliases, route decisions,
  fallback history, cost/latency attribution, and audit correlation.
- OpenAPI/JSON Schema, event fixtures, and compatibility conformance tests.

## Acceptance criteria

- OID and an external assistant can use the same canonical request/stream
  semantics through different API edges.
- Unsupported features fail explicitly and cannot be silently emulated.
- A route response explains selected and rejected candidates and preserves
  local-only/provider-pinned policy.
- Tool calls remain proposals until a governed host authorizes execution.

## Dependencies

WP-0057, WP-0058, WP-0060, WP-0061, WP-0067; ADR-0011, ADR-0013, ADR-0015.
