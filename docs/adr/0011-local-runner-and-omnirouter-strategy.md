# ADR-0011: OID-Owned Local Runner and Multi-Backend Router Strategy

- Status: Accepted
- Date: 2026-10-01
- Related: ADR-0003, ADR-0006, ADR-0007, ADR-0010; WP-0057 through WP-0063

## Decision

Marina will provide an Ollama-like local runner experience through an
OID-owned, versioned runtime API and will be usable as a standalone service.
OID will not embed Ollama as its control-plane abstraction; OID and external AI
assistants consume Marina as clients. A router will select among local and
remote backend adapters using declared capabilities, model identity, policy,
health, resource fit, cost, latency, quotas, and caller preferences.

```text
OID / assistant / system client
            -> Marina API edge
            -> policy/admission -> Router -> Backend adapter
                                      |       |-> llama.cpp (local first)
                                      |       |-> Ollama / Docker Model Runner edge
                                      |       `-> OpenAI-compatible remote edge
                                      `-> auditable decision/evidence
```

The first production slice remains local, CPU-first, one active model, and
GGUF-focused. Remote providers, multi-model scheduling, tools, embeddings,
RAG, and autonomous agent loops are separate capabilities.

## Required capabilities

- Stable model manifests and verified artifact/cache state.
- Load/unload/list/inspect/pull/copy/delete, streaming generation, chat,
  cancellation, health, and request metrics with stable IDs.
- Model templates/parameters, keep-alive, structured output, tool-call
  proposals, embeddings, and multimodal inputs when an adapter declares them.
- Standalone service installation and lifecycle management on Linux, macOS,
  and Windows.
- OpenAI-compatible and Ollama-compatible API edges, with explicit capability
  gaps and stable error mapping.
- Capability negotiation for context size, streaming, structured output,
  embeddings, tool calls, and modality support.
- Admission checks for authorization, model trust, context, memory, device,
  concurrency, and quotas.
- Explainable routing with candidates, rejection reasons, fallback status, and
  policy/version identifiers, cost/latency estimates, quota state, and circuit
  breaker state.
- Provider credentials, aliases, retries, fallbacks, rate limits, and local /
  offline routing policies without leaking credentials to clients.
- Failure isolation and recovery for crashes, disconnects, timeouts,
  cancellation, and partial streams.
- Compatibility adapters at the edge; provider-native types never enter the
  OID core API.

## Routing rules

Routing is deterministic for the same request, policy, registry, and health
snapshot. Local backends are preferred when policy and capacity allow. Remote
backends require explicit opt-in, isolated credentials, endpoint health, and
cost attribution. Fallback never silently changes data-handling policy or
model identity; a changed backend is visible in the response and audit record.

## Alternatives considered

- **Ollama as the permanent runtime:** rejected because it hides lifecycle,
  resources, policy, and routing details OID must govern.
- **Gateway first:** rejected because routing without a reliable local runner,
  manifest, and admission model creates opaque failure and cost behavior.
- **Provider-specific clients in the console:** rejected because it couples UX
  and future assistants to backend protocols.
- **Build a custom inference engine:** rejected; OID owns orchestration, not
  tensor kernels or model mathematics.

## Consequences

The project maintains a rich internal OID contract and deliberately limited
compatibility edges. The router adds policy and observability work, but enables
backend replacement and future assistant composition without rewriting clients.
The initial one-model constraint remains intentional until admission and
scheduling are real.

This ADR is a target decision, not a claim that all capabilities are complete.
WP-0057 through WP-0068 provide the implementation and validation sequence.
