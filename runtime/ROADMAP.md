# Intelligent Runtime Roadmap

This roadmap sequences the control plane from the current local runner to an
Ollama-like service and OID-owned multi-backend router. Milestones are
capability boundaries, not promises of implementation in the current
repository. See `docs/PROJECT-OPERATING-BRIEF.md` and ADR-0011.

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

The existing local runner is beyond contract-only status: native GGUF loading,
streaming, cancellation, and basic metrics have validation evidence. The next
work is WP-0057 through WP-0063, in order: contract hardening, trust/admission,
service transport, routing, compatibility, recovery, then assistant sessions.

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

## v0.4 — Standalone cross-platform Marina service

Goal: ship Marina as an independently installable service that works on Linux,
macOS, and Windows and can be consumed by OID, AI assistants, and other
applications.

- Deliver user/system installers and architecture-aware variants (WP-0065).
- Provide portable local service transports and a documented API (WP-0066).
- Add a loopback-first assistant/OpenAI-compatible streaming edge (WP-0067).
- Integrate OID through the public API, preserving policy and evidence (WP-0068).
- Stabilize the canonical AI capability/provider contract and conformance suite
  (WP-0070).
- Add runtime security, resource isolation, crash recovery, and provider/model
  provenance controls (WP-0071).

Exit criteria: a clean machine can install Marina without OID, a non-OID
assistant can make a streaming request, and OID can use the same service
without reading model files or constructing backend handles. Cross-platform
user installation and standalone service behavior must be stable before distro
packaging begins.

## v0.5 — Linux distro delivery

Goal: finish Marina as a distro-quality Linux product after the portable
runner, service, API, router, and isolation contracts are stable.

- Ship Debian/Ubuntu and Fedora/RHEL packages with architecture variants
  (WP-0069).
- Add systemd user/system units, FHS paths, secure IPC, migration, rollback,
  uninstall/purge, and offline installation.
- Validate clean installs and upgrades on the supported distro matrix.

Exit criteria: Linux package and service checks pass in both user and system
installation modes without a development checkout or OID dependency.

## v1.0 — Production-grade runtime service

Goal: provide a stable, secure, and operable model resource service.

- Stabilize the versioned API and compatibility policy.
- Complete crash recovery, upgrade, cache migration, and rollback procedures.
- Harden authorization, TLS, secrets handling, quotas, audit retention, and tenant isolation.
- Publish operational SLOs, capacity guidance, threat model, and support matrix.
- Define OIP integration contracts without coupling the runtime to OIP deployments.

Exit criteria: production readiness review passes for security, reliability, observability, API compatibility, and documented backend support.

## Parallel product track — Marina training and model lifecycle

Training is a governed lifecycle capability layered after the inference
contracts, native service API, request lifecycle, and resource admission; it
is not a prerequisite for the first working runner. WP-0077 and ADR-0021 must
complete before WP-0076/ADR-0020:

- Deliver the native HTTP server and versioned API.
- Persist listener configuration while preserving Unix-socket and Windows
  loopback defaults.
- Add authentication, scopes, cancellation, timeouts, quotas, and admission.
- Integrate OID as a genuine Marina client and retain IDE compatibility.

Then WP-0076 and ADR-0020 define the training sequence:

- Establish backend-neutral training, dataset, artifact, lineage, and job-state
  contracts.
- Execute a bounded CPU-compatible small-model training proof.
- Add dataset/model hashes, checkpoints, evaluation, baseline comparison,
  promotion approval, activation, and rollback.
- Add hardware/resource eligibility, worker isolation, inference priority, and
  training evidence.
- Integrate LoRA/QLoRA only through replaceable training backends.

Training must never automatically replace an active inference model or turn
prompts, logs, or operating-system observations into training data.

## v1.x — AI desktop/OS agency

Goal: make OID the governed control plane users can prompt to operate and
maintain the computer.

- Build the desktop/system context graph and privacy/freshness controls.
- Add typed desktop, filesystem, process, package, service, device, and
  diagnostics skills.
- Add risk-aware approvals, privilege brokering, bounded execution,
  verification, rollback, and recovery.
- Add policy-approved self-maintenance, notifications, evidence views, and
  safety/task evaluations (WP-0072).

Exit criteria: supported computer tasks produce inspectable plans, execute only
within explicit authority, verify their outcomes, and recover safely when a
step fails or the model is unavailable.

## Parallel product track — OID OS-management model

This track develops the specialized model that powers OID agency. It may begin
with a specialized checkpoint or fine-tune while the runner and desktop
control plane mature; a new base model is a separate research decision.

- Define governed context, plan, tool-proposal, approval, verification, and
  recovery datasets.
- Establish reproducible training/fine-tuning, provenance, licensing, and
  model-card workflows.
- Evaluate safe action, refusal, approval compliance, recovery, latency,
  memory, and offline behavior.
- Publish the selected model as a Marina-served manifest and variant (WP-0073).

Exit criteria: the model emits validated typed proposals, survives adversarial
evaluation without bypassing OID authority, and can be served by Marina as a
versioned, rollback-compatible model.

## Linux product track — Rust CA-Clipper subsystem

The legacy CA-Clipper archive is a separate Linux product input, not a Marina
backend. WP-0074 first inventories and legally clears the archive, then builds
a Rust/Linux vertical slice, data compatibility layer, and governed OID
integration before adding it to distro packaging.
