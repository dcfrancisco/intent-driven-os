# ADR-0015: AI Capability, Provider, and Assistant Contract

- Status: Accepted
- Date: 2026-10-07
- Related: ADR-0003, ADR-0006, ADR-0011, ADR-0013; WP-0057, WP-0060, WP-0067, WP-0070, WP-0071

## Decision

Marina has one canonical, versioned AI request contract. Ollama-compatible,
OpenAI-compatible, Anthropic-compatible, MCP, and future A2A surfaces are
translation edges. They must not become separate internal execution models.

The canonical contract represents:

- model identity, revision, source, trust state, and selected backend;
- caller identity, tenant/project scope, request ID, session ID, and policy
  version;
- ordered messages and typed content parts (text, image, audio, tool result);
- generation controls, context budget, timeout, cancellation, and keep-alive;
- declared requirements for streaming, structured output, tools, embeddings,
  vision, audio, reasoning, and other capabilities;
- tool-call proposals as data, never implicit privileged execution;
- usage, latency, cost attribution, route explanation, and terminal outcome;
- structured errors for unsupported capability, policy denial, quota, provider,
  timeout, cancellation, and partial-stream conditions.

## Model and provider separation

A model is a versioned logical resource. A provider/backend is an execution
implementation. One model may have multiple compatible variants; one provider
may serve multiple models. The registry records capabilities and constraints;
the router selects an admissible model/provider pair.

Provider credentials are references to an OS or deployment secret store. They
must not appear in model manifests, prompts, logs, client responses, or source
configuration. Local/offline policy is explicit and must prevent accidental
remote fallback.

On a shared machine, Marina owns the model catalog, artifact store, loaded
model state, and backend handles. OID is a privileged local API client for
governed system workflows, not a second model owner. OID may request model
listing, admission, generation, cancellation, and metrics through its local
principal; it must not bypass Marina by opening model files directly.

## Routing contract

Routing is a deterministic admission decision over candidates. Candidate
evaluation includes capability fit, model revision, context and memory fit,
hardware, health, latency, cost, quota, privacy/data policy, retry state, and
circuit-breaker state. A route response includes the selected candidate,
rejected candidates and reasons, policy version, and fallback history.

Retries and fallbacks are bounded and operation-aware. They may not duplicate a
non-idempotent tool request or silently move data from local to remote. A
caller can request local-only, provider-pinned, or policy-approved automatic
routing.

## Assistant and tool boundary

Marina owns session transport, context limits, streaming, model calls, and
tool-call proposals. OID and other governed hosts own tool registration,
authorization, approval, execution, verification, rollback, and evidence for
system actions. A model response cannot directly execute shell, filesystem,
network, or privileged operations.

MCP/A2A adapters expose bounded capabilities and preserve the same caller,
request, policy, route, and evidence identifiers. They are not alternate
authorization paths.

## Compatibility and evolution

Each compatibility edge publishes a capability matrix and conformance tests.
Unsupported parameters are rejected or explicitly reported as ignored; they
are never silently approximated. API versions, model IDs, event schemas, and
error codes have deprecation rules. OpenAPI/JSON Schema and streaming fixtures
are release artifacts.

## Consequences

The runner grows into an AI service platform rather than a CLI wrapper, but
feature expansion remains bounded by capability declarations and policy. Chat
history, embeddings, multimodal inputs, structured output, tools, provider
routing, evaluation, and compatibility require explicit implementation slices;
the current text-generation runner is not evidence that they already exist.

## Current implementation gap register

The present implementation has a local Unix-socket development daemon and a
tab-delimited command protocol. It does not yet implement the canonical API,
HTTP compatibility edges, chat history/session persistence, embeddings,
multimodal content, structured-output validation, tool-call schemas, provider
credentials, route explanations, quotas, or MCP/A2A authorization. These are
tracked as implementation work, not implied by this ADR.

The first usable IDE milestone may use the dependency-free
`scripts/marina-openai-proxy.py` bridge. It terminates HTTP on loopback,
requires a Marina bearer token, translates a small `/v1/models` and
`/v1/chat/completions` subset to the local daemon, and does not provide remote
provider routing. It is a compatibility bridge, not the final canonical API.
BYOK provider credentials must remain in the IDE or an OS secret store until a
provider adapter and credential contract are implemented.

The minimum AI-service release gate is:

1. canonical JSON Schema/OpenAPI request, response, event, and error contracts;
2. capability discovery and conformance tests for every compatibility edge;
3. stable request/session/route IDs and resumable or terminal stream semantics;
4. model/provider manifests with trust, license, context, modality, tool,
   embedding, and resource capabilities;
5. deterministic route explanations and bounded retries/fallbacks;
6. session retention, prompt/data redaction, credential isolation, and audit;
7. independent evaluation fixtures for quality, latency, cost, tool safety,
   and refusal/unsupported-capability behavior.
