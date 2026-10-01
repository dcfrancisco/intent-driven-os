# Project Operating Brief

This is the compact context packet for a new human or AI assistant. It is
intended to be read once per task, not repeated in every prompt.

## Objective

Build a Linux-first Rust runtime and console that makes local intelligence a
managed, observable, policy-controlled system resource. The first useful
runtime must feel as simple as an Ollama-like local runner while retaining an
OID-owned control plane. The next layer is an OmniRouter-like gateway that
chooses among local and remote backends without leaking provider details to
clients.

## Architecture in one line

`client -> versioned OID API -> policy/admission -> model registry/cache -> router -> backend adapter -> stream/events/evidence`

OID owns the API, model identity, lifecycle, request IDs, admission, routing
decisions, cancellation semantics, metrics, and audit trail. Adapters own only
translation to llama.cpp, Ollama, OpenAI-compatible, or future engines.

## Inventory snapshot (2026-10-01)

Implemented or substantially present:

- Rust workspace with separated runtime, console, model-runner, policy,
  evidence, verification, operation-coordinator, and plugin boundaries.
- Direct opt-in llama.cpp FFI boundary and GGUF model loading.
- Model discovery, model registry view, one active model, tokenization,
  streaming generation, cooperative cancellation, and basic statistics.
- Marina daemon plus local `marinactl` Unix-socket boundary.
- The runner can be installed independently as `marina` plus `marinactl`; the
  interactive console is optional.
- Governed operation lifecycle with approval, recovery, verification, rollback,
  evidence, native shell passthrough, and read-only Linux skills.

Not complete:

- Persistent verified model cache/download/pull and manifest trust lifecycle.
- Resource reservations, context admission, multi-model residency, scheduling,
  GPU/NPU placement, quotas, rate limiting, and crash supervision.
- Versioned public API/compatibility translation, backend capability negotiation,
  and a real multi-backend router.
- Chat/session state, tool execution, embeddings, RAG, and autonomous agent
  loops.
- Production packaging, real-Linux interruption/recovery validation, and a
  stable compatibility/support matrix.

## Target path

1. **Runner hardening:** make one local GGUF model reliable and measurable.
2. **Local service API:** expose model/run/stop/health/stream semantics over the
   Unix socket with versioning and idempotency.
3. **Resource control:** add manifest verification, admission, memory/context
   accounting, lifecycle recovery, and deterministic model selection.
4. **Router:** introduce capability-based backend selection, fallback policy,
   health/circuit breaking, and explainable routing decisions.
5. **Compatibility:** add a deliberately scoped Ollama/OpenAI-compatible edge
   without making either protocol the internal model.
6. **Assistant composition:** add session context, governed tools, and agent
   orchestration only after the runtime lifecycle and policy boundaries are
   reliable.

The installation track is WP-0064: package the daemon/client independently,
preserve user-scoped state, and keep native backend setup optional at install
time.

## Definition of done for the runner/router track

- A clean Linux machine can install/start the runtime with no model present.
- A verified local model can be listed, loaded, streamed, cancelled, unloaded,
  and inspected through the same versioned API.
- A request cannot exceed declared context/memory/policy limits.
- Every run has a stable request ID, terminal outcome, metrics, and audit record.
- Backend failures do not corrupt runtime state; recovery is deterministic.
- Routing returns the selected backend plus a machine-readable explanation.
- Compatibility tests prove supported Ollama/OpenAI behavior and explicitly
  reject unsupported semantics.
- Native shell and governed-operation behavior remain intact without a model.

## Collaboration contract

Use `AGENTS.md` for agent lanes and handoff rules. Keep ADRs for decisions,
WPs for bounded implementation slices, and code/tests for executable truth.
When an agent discovers a mismatch, update the relevant inventory/plan rather
than leaving the next assistant to rediscover it.
