# Intelligent Runtime Roadmap

This roadmap sequences the control plane from documented contracts to a usable local runtime. Milestones are capability boundaries, not promises of implementation in the current repository.

## v0.1 — Contracts and local foundation

Goal: establish a safe, observable local runtime boundary around one backend.

- Define model IDs, manifests, backend capabilities, device resources, run IDs, lifecycle states, and typed errors.
- Specify model registry, local cache, SHA256 verification, and artifact trust states.
- Define hardware discovery for CPU, GPU, and NPU, including unknown-device handling.
- Define memory budgets, context limits, quantization selection inputs, and admission decisions.
- Define backend adapter interface and the initial `llama.cpp` adapter boundary.
- Define streaming inference and cancellation semantics.
- Define local Unix socket transport and CLI-to-API mapping.
- Establish structured audit events, health states, metrics, and tracing fields.

Exit criteria: contracts are reviewed, lifecycle invariants are documented, and a backend-neutral request can be described end to end without implementation leakage.

## v0.2 — Resource and policy control

Goal: make model execution predictable under resource contention.

- Add memory accounting, reservations, pressure handling, idle model unloading, and context enforcement.
- Add device placement and GPU scheduling policy with priorities and concurrency limits.
- Add authentication, authorization, resource quotas, and rate limiting.
- Add model health monitoring, crash recovery, and backend restart policy.
- Add model pull/inspect/list and backend list/enable control-plane operations.
- Define optional TCP transport boundaries and TLS requirements.

Exit criteria: authorized clients can reason about placement, resource use, failure, and recovery using observable runtime state.

## v0.3 — Compatibility and extensibility

Goal: broaden integration without weakening the common boundary.

- Add OpenAI-compatible request and streaming translation.
- Add a second local or managed backend after capability-gap review.
- Define plugin discovery, versioning, signing/trust, isolation, and lifecycle.
- Add policy profiles for shell, desktop, background service, and application callers.
- Add richer metrics, tracing, audit export, and operational health endpoints.
- Validate remote OpenAI-compatible providers as a distinct security and billing domain.

Exit criteria: at least two backend classes can implement the same caller workflow, and extension/compatibility behavior is versioned and documented.

## v1.0 — Production-grade runtime service

Goal: provide a stable, secure, and operable model resource service.

- Stabilize the versioned API and compatibility policy.
- Complete crash recovery, upgrade, cache migration, and rollback procedures.
- Harden authorization, TLS, secrets handling, quotas, audit retention, and tenant isolation.
- Publish operational SLOs, capacity guidance, threat model, and support matrix.
- Define OIP integration contracts without coupling the runtime to OIP deployments.

Exit criteria: production readiness review passes for security, reliability, observability, API compatibility, and documented backend support.

