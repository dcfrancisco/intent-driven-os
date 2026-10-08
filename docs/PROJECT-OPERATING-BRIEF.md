# Project Operating Brief

This is the compact context packet for a new human or AI assistant. It is
intended to be read once per task, not repeated in every prompt.

## Objective

Build a Linux-first Rust AI desktop/OS that makes local intelligence a
managed, observable, policy-controlled system resource. The first useful
runtime must feel as simple as an Ollama-like local runner while retaining an
OID-owned control plane. The product goal is that a user can prompt the
computer to inspect, plan, operate, verify, recover, and maintain itself. The
next layers are an OmniRouter-like gateway and governed desktop/system agency,
without leaking provider details or granting models direct authority.

## Architecture in one line

`client -> Marina API edge -> policy/admission -> model registry/cache -> router -> backend adapter -> stream/events/evidence`

Marina owns the public API, model identity, lifecycle, request IDs, admission,
routing decisions, cancellation semantics, metrics, and audit trail. OID is a
governed client and system integration. Adapters own only translation to
llama.cpp, Ollama, Docker Model Runner, OpenAI-compatible, or future engines.

## Inventory snapshot (2026-10-01)

Implemented or substantially present:

- Rust workspace with separated runtime, console, model-runner, policy,
  evidence, verification, operation-coordinator, and plugin boundaries.
- Direct opt-in llama.cpp FFI boundary and GGUF model loading.
- Model discovery, model registry view, one active model, tokenization,
  streaming generation, cooperative cancellation, and basic statistics.
- Marina daemon plus local `marinactl` Unix-socket boundary.
- Native loopback HTTP API for health, model discovery/load, chat/completions,
  bearer authentication, streaming, and request cancellation; the Python
  bridge remains a migration edge.
- The runner can be installed independently as `marina` plus `marinactl`; the
  interactive console is optional.
- Governed operation lifecycle with approval, recovery, verification, rollback,
  evidence, native shell passthrough, and read-only Linux skills.

Not complete:

- Persistent verified model cache/download/pull and manifest trust lifecycle
  beyond the current GGUF import path.
- Resource reservations, context admission, multi-model residency, scheduling,
  GPU/NPU placement, quotas, rate limiting, and crash supervision.
- Versioned public API/compatibility translation, backend capability negotiation,
  and a real multi-backend router.
- Chat/session state, tool execution, embeddings, RAG, and autonomous agent
  loops.
- Cross-platform production packaging/service installation, public HTTP/API
  compatibility, real-Linux interruption/recovery validation, and a stable
  compatibility/support matrix.

## Target path

1. **Runner hardening:** make one local GGUF model reliable and measurable.
2. **Standalone service:** install Marina independently and expose model/run/
   health/stream semantics over portable local transports.
3. **Resource control:** add manifest verification, admission, memory/context
   accounting, lifecycle recovery, and deterministic model selection.
4. **Router:** introduce capability-based backend selection, fallback policy,
   health/circuit breaking, quotas, cost/latency policy, and explainable
   routing decisions.
5. **Compatibility:** add deliberately scoped Ollama, OpenAI, and later
   Anthropic-compatible edges without making any protocol the internal model.
6. **Assistant composition:** add session context, governed tools, MCP/A2A
   integration, and agent orchestration only after runtime lifecycle and
   policy boundaries are reliable.
7. **AI desktop/OS agency:** add context graphs, typed desktop/system skills,
   approval and privilege brokering, verification, rollback, self-maintenance,
   notifications, and safety evaluation (WP-0072).
8. **OID model track:** develop and evaluate a specialized OS-management model,
   then serve it through Marina with typed proposals and no direct authority
   (WP-0073).
9. **Linux application subsystem:** migrate the separate CA-Clipper project to
   a governed Rust/Linux subsystem for the OID distro after inventory,
   provenance, licensing, and data-compatibility gates (WP-0074).
10. **Marina model lifecycle:** add governed dataset registration, training,
    fine-tuning, evaluation, promotion, and rollback without coupling training
    dependencies to inference (WP-0076, ADR-0020).
11. **Marina native service API:** replace the Python bridge as a production
    dependency with native HTTP, persistent listener configuration,
    authentication, request lifecycle controls, OID client integration, and
    resource admission before training begins (WP-0077, ADR-0021).

The installation and productization track is WP-0064 through WP-0071: package
the model runner daemon/client independently for Linux, macOS, and Windows,
preserve user-scoped state, and expose the same API to OID, AI assistants, and
system clients. OID itself and the AI desktop/OS remain Linux-first. Linux
distro integration is the
final delivery phase, after the portable runner, API, router, provider
contract, and resource-isolation work are stable. Its distro packaging and
system-service requirements remain release gates for Linux, but do not block
the earlier standalone product milestones.

## Minimum working model-runner path

The minimum path for a usable Marina runner is:

1. WP-0057 — harden request, stream, cancellation, metrics, and model
   contracts.
2. WP-0058 — verify model artifacts and enforce basic admission.
3. WP-0059 — expose a versioned local service boundary.
4. WP-0071 — add local authentication, secret boundaries, and resource
   isolation.
5. WP-0067 — provide the IDE-facing OpenAI-compatible edge.
6. WP-0068 — make OID consume the same Marina service and model inventory.

WP-0061 and WP-0070 expand compatibility and provider routing after the local
path is reliable. WP-0069 is Linux distro delivery, not a prerequisite for the
first usable runner.

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
