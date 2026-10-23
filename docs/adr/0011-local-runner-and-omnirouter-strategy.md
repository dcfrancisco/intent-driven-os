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

The first production slice remains local, CPU-first, one active generation,
and GGUF-focused. Multiple resident models and native embeddings are now part
of the bounded Talvik M1 slice; concurrent scheduling, remote providers, tools,
RAG, and autonomous agent loops remain separate capabilities.

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

Routing is optional and disabled by default. The direct model-selection path
must remain available. The first productized slice may provide explicit,
ordered local-model fallback at the client boundary, provided it reports each
candidate, never falls back after streaming begins, and cannot enable itself.
Capability-aware backend/provider routing and durable route evidence remain
gated by WP-0060.

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
The initial one-model implementation constraint was removed for Talvik M1 after
aggregate memory admission and explicit model selection were implemented and
validated. One active generation remains intentional until concurrent
scheduling is designed and qualified.

This ADR is a target decision, not a claim that all capabilities are complete.
WP-0057 through WP-0068 provide the implementation and validation sequence.

The lifecycle closeout also requires terminal-event ordering: a completed,
cancelled, or failed generation must release its active-generation admission
slot before the terminal response is observable to the client. This is an
implementation invariant under WP-0059, not a change to the backend-neutral
architecture.

## CPU-first closeout audit — 2026-10-17

The final build verified embedded-template application and uninterrupted
decoded real-GGUF output. The milestone remains open: final-binary independent
request isolation and the cancellation/recovery/unload/reload/shutdown socket
sequence did not complete and must not be inferred from unit tests.

## Superseding implementation note — 2026-10-17

The first native runner implementation passed raw prompts directly to the
backend even when a GGUF contained `tokenizer.chat_template`. A real Qwen
GGUF exposed this as unrelated instruction output despite successful model
loading and process completion. The llama.cpp adapter now obtains and applies
the embedded template for a single user message before tokenization, while
retaining raw-prompt behavior for template-less models. This keeps prompt
semantics inside the model execution adapter, preserves the backend-neutral
runtime boundary, and makes prompt correctness a release acceptance property.

## M1 implementation amendment — 2026-10-23

Talvik M1 extends resident model state from a singleton to an ordered map of
model IDs. Generation requests select a loaded model explicitly, aggregate
resident memory participates in admission, and unloading one model preserves
the others. Native handles remain inside the llama.cpp adapter. The amendment
also adds native embeddings and requires unsupported tool requests to fail
explicitly at the compatibility edge; it does not authorize tool execution or
concurrent generation.
