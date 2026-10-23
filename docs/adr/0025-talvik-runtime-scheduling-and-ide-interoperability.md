# ADR-0025: Talvik Runtime Scheduling, Resource Governance, and IDE Interoperability

- Status: Accepted for M2 implementation
- Date: 2026-10-23
- Related: ADR-0003, ADR-0004, ADR-0006, ADR-0007, ADR-0011, ADR-0013,
  ADR-0021; WP-0058, WP-0061, WP-0062, WP-0067, WP-0091, WP-0092, WP-0093,
  WP-0094

## Context and evidence

Talvik M1 closed its CPU-first development scope with multiple resident
llama.cpp GGUF models, one active generation, bounded FIFO admission, LRU
recovery, pinning, authentication, and an OpenAI-compatible edge. The current
implementation is in `runtime/src/service.rs`, `runtime/src/models.rs`, and
`console/src/bin/http.rs`. M1 qualification evidence is consolidated in
`docs/TALVIK-M1-CLOSEOUT.md` and `docs/TALVIK-M1-ACCEPTANCE.md`.

The local Ollama reference was inspected at
`/Users/dannyfrancisco/codes/ollama`, commit
`f52b9e742d8c9a308f7b23cd1f0ab52c7ddb694d`. Relevant evidence is in
`server/sched.go` and `server/routes.go`. Ollama uses a keyed loaded-runner
map, serializes loading while allowing requests to an already loaded runner,
tracks active references, bounds pending requests, and expires idle runners
through keep-alive timers. Its `/api/tags`, `/api/show`, `/api/ps`, `/api/chat`,
and `/api/generate` contracts are documented in `docs/api.md` and
`docs/openapi.yaml`.

## Current Talvik comparison

- **Implemented and verified:** resident model map, explicit model identity,
  active-request protection, FIFO queue, bounded LRU eviction, pinned models,
  fail-closed admission, reload, cancellation, OpenAI chat/completions/SSE,
  embeddings, and capability-gated tool calls.
- **Implemented but incompletely qualified:** native multi-model behavior is
  recorded on macOS x86_64 only; other hosts and real IDE clients remain open.
- **Missing:** externally useful resident-model diagnostics equivalent to a
  concise `/api/ps` view, documented keep-alive/idle retention behavior at the
  compatibility edge, and a real Continue/Cline qualification result.
- **Intentionally out of scope:** GPU scheduling, concurrent generation,
  remote-provider routing, tool execution, agent loops, and MarinaOS.

## Decision

Keep Talvik's simpler one-active-generation policy for M2 while making
residency and scheduling observable and client-usable. Add only a justified
Ollama-compatible subset: `/api/tags` for model discovery and `/api/ps` for
actual resident-model inspection. Do not add compatibility endpoints that do
not serve a demonstrated client workflow. Preserve `/v1/*` as the primary
assistant edge.

Expose scheduler state from runtime-owned snapshots rather than leaking native
handles. Report only measured or configured values; unknown memory remains
unknown. Keep-alive/idle retention remains a runtime policy, with explicit
unload and automatic eviction preserving active-request and pinning invariants.

## Invariants and security

1. A resident model is not unloaded while an active request references it.
2. Queued cancellation removes execution eligibility before dispatch.
3. Every terminal request outcome releases its active slot and reservation.
4. Requested model identity is never silently substituted.
5. Authentication and scope checks happen before protected inference work.
6. Diagnostics contain no prompt or generated-response content by default.
7. Local HTTP remains loopback-first and bearer-authenticated by default.
8. Backend-native types remain inside the llama.cpp adapter.

## Alternatives rejected

- Replacing Talvik's scheduler with Ollama's implementation: rejected because
  Talvik must retain its backend-neutral Rust boundary and CPU-first scope.
- Enabling concurrent generation now: rejected until native context ownership,
  memory reservations, and measured latency/RAM evidence justify it.
- Implementing all Ollama endpoints: rejected as compatibility surface without
  a client or workflow need.
- Claiming IDE interoperability from OpenAI-client tests: rejected; a real IDE
  session is required for PASS.

## Qualification and consequences

M2 must verify resident diagnostics, model switching, queue/cancellation
invariants, authentication, and API compatibility with deterministic tests and
real GGUF runs where available. Continue/Cline remains an owner-deferred
external gate until a real client session is available. Windows, Linux,
macOS arm64, and Docker remain platform-specific gates and cannot be inferred
from cross-compilation.

## Initial implementation slice — 2026-10-23

WP-0091 is implemented on `feature/talvik-m2-runtime-ide`: runtime snapshots
now expose resident model memory/idle data, pending and active request counts,
queue capacity, eviction state, and retention policy. The HTTP edge provides
authenticated `GET /api/tags` and `GET /api/ps` backed by those snapshots.
Deterministic runtime and HTTP contract tests pass; real-GGUF and real-IDE
qualification remain separate acceptance obligations.

## Capability-register policy

The complete Ollama comparison is maintained in
`docs/TALVIK-OLLAMA-CAPABILITY-MATRIX.md`. Additional Ollama routes are not
implemented merely for endpoint-count parity: `/api/show`, `/api/chat`,
`/api/generate`, and `/api/embed` are owned by WP-0096 and require evidence of
a selected client need. GPU scheduling, multimodal support, MarinaOS service
integration, and alternate inference engines are later-milestone work owned by
WP-0100 through WP-0103. This preserves the llama.cpp-only, CPU-first boundary.
