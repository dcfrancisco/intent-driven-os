# Talvik / Intelligent Runtime Backlog

Items are intentionally written as design and implementation slices for future work. Nothing in this backlog is implemented by this documentation module.

## Non-blocking product identity clearance

- [ ] `TALVIK-NAME-001` (P2, owner: product/legal): perform legal trademark,
  package-registry, source-repository, social-handle, and domain clearance for
  **Talvik** in intended release regions. Dependencies: WP-0075 and a defined
  launch-region list. Acceptance: dated evidence and an approve/rename
  disposition are linked from ADR-0019 before a public stable-name release.
  This finding does **not** block engineering, CPU-first qualification, or use
  of Talvik as the provisional internal/product-development identity.
- [ ] Define the eventual `$HOME/.talvik` migration only after name clearance.
  Preserve `$HOME/.marina` in place until an atomic migration, rollback, and
  compatibility window are specified and tested.

## Current execution sequence

The repository now has a validated CPU-first local GGUF runner. Execute the
remaining runtime work through the bounded packages in
`docs/work-packages/README.md`: WP-0057 (contract hardening), WP-0058
(verified cache/admission), WP-0059 (versioned service transport), WP-0060
(capability router), WP-0061 (compatibility edges), WP-0062 (recovery and
observability), and finally WP-0063 (assistant sessions/tools). This sequence
keeps assistant behavior above, rather than inside, model backends.

The product distribution sequence continues with WP-0065 through WP-0071:
cross-platform packaging, standalone service transports, the assistant API
compatibility edge, OID integration, the canonical AI capability contract, and
runtime isolation. Linux distro/service integration (WP-0069) is deliberately
the final delivery phase after those contracts are stable. Marina is the
standalone service; OID, assistants, and system workflows are clients of its
public API.

### Regression gate completed 2026-10-17

- [x] Apply the embedded GGUF chat template before tokenization when present.
- [x] Preserve raw-prompt fallback for models without a chat template.
- [x] Rebuild and reproduce the corrected behavior with a real Qwen GGUF.
- [x] Verify streaming, cancellation, independent requests, unload/reload,
      and post-reload inference on the corrected binary.

Closeout audit 2026-10-17: the final decoder build passed uninterrupted
real-GGUF completion, embedded-template formatting, and workspace tests.
Independent final-binary socket/lifecycle acceptance remains open because the
socket harness blocked before its first token; do not mark the CPU-first
milestone complete until that sequence passes.

Lifecycle follow-up 2026-10-17: `MARINA-F-001` (harness newline framing) and
`MARINA-F-002` (terminal-before-slot-release race) are resolved and verified
by `scripts/accept-marina-lifecycle.py`. `MARINA-F-003` remains open under
WP-0056 for four process-signal tests that timed out before the console-ready
marker.

Productization follow-up 2026-10-17: `TALVIK-F-001` reproduced a second
terminal-ordering race in the llama.cpp adapter's private admission flag. It
is RESOLVED by releasing adapter admission before terminal publication; the
corrected release binary passed the full checked-in lifecycle harness. Evidence
is linked from the model-runner reality review.

The acceptance evidence and the original failed output are retained in
`docs/reviews/model-runner-reality-review.md`. Exact semantic output remains a
sampling-controlled qualification concern, not a process-exit assertion.

### Talvik M1 implementation completion — 2026-10-23

- [x] Multi-model resident map with explicit model selection, aggregate memory
  admission, independent unload, and post-unload recovery.
- [x] CLI help, JSON status/model output, and actionable request errors.
- [x] Native embeddings path and OpenAI-compatible `/v1/embeddings` edge.
- [x] Multi-turn role preservation through the embedded GGUF chat template.
- [x] Capability-gated GGUF tool calls with normalized OpenAI `tool_calls` and
  assistant/tool-result round trips.
- [x] Fresh real-GGUF lifecycle validation with release binaries.
- [x] Resumable acquisition retains `.part` files, reports progress, retries
  HTTP transfers, verifies trusted SHA-256 values, and records observed
  checksums in the registry.
- [ ] Windows and macOS arm64 native qualification remain WP-0079/WP-0080.

### Talvik M2 starting backlog — 2026-10-23

- [ ] WP-0091: expose truthful resident-model and scheduler diagnostics and
  justify the `/api/tags` and `/api/ps` compatibility subset.
- [ ] WP-0092: qualify a real Continue/Cline coding workflow; Codex support is
  documented only where the installed client permits it.
- [ ] WP-0093: make keep-alive/idle retention and resource-pressure behavior
  explicit without changing one-active-generation safety.
- [ ] WP-0094: measure concurrent inference before authorizing implementation;
  do not infer safety from multi-model residency.
- [ ] WP-0095: qualify consistent CLI, daemon, IPC, and HTTP build identity.
- [ ] WP-0096: retain the Reference local runner endpoint gap register; implement additional
  routes only when a real client requires them.
- [ ] WP-0097: qualify model profiles, aliases, metadata inspection, and
  registry deduplication without introducing OCI distribution.
- [ ] WP-0100/0101/0102/0103: schedule GPU, multimodal, MarinaOS integration,
  and alternate-engine decisions for later milestones.

## v0.1 — Contracts and local foundation

### Registry and models

- [ ] Define stable model identity, version, source, license, capabilities, and ownership fields.
- [ ] Define the model manifest schema, including artifact URLs, SHA256 digests, format, context limits, quantization variants, and backend compatibility.
- [ ] Define registry states: discovered, trusted, cached, unavailable, revoked, and deprecated.
- [ ] Define model cache layout, atomic writes, eviction metadata, and cache corruption handling.
- [ ] Define SHA256 verification and the trust decision recorded for every artifact.

### Hardware and resources

- [ ] Define CPU, GPU, and NPU discovery records and capability normalization.
- [~] Define memory budgets, reservations, accounting units, and pressure signals. Talvik now has bounded CPU-first admission/recovery policy; direct llama.cpp KV/compute allocation accounting remains open under WP-0058.
- [ ] Define quantization selection inputs and deterministic fallback rules.
- [ ] Define resource requests for model load, active run, context, and streaming buffers.

### Backend and execution

- [ ] Define backend adapter interface, capability negotiation, versioning, and structured errors.
- [ ] Define `llama.cpp` adapter responsibilities and the boundary to its native API.
- [ ] Define model load, unload, run, stop, health, and cancellation semantics.
- [ ] Define streaming events, ordering, backpressure, disconnect, and resume behavior.

### Local API and operations

- [ ] Define versioned local API resources and idempotency rules.
- [ ] Define Unix socket transport, permissions, peer identity, and socket lifecycle.
- [ ] Map future CLI commands: `model list`, `pull`, `inspect`, `run`, `stop`, `health`, `hardware`, `backend list`, and `backend enable`.
- [ ] Define lifecycle audit events, health states, metric names, and trace correlation fields.

## v0.2 — Resource and policy control

- [x] Specify memory manager behavior under admission failure and memory pressure for the CPU-first runtime slice (WP-0058/TALVIK-F-012); accelerator policy remains future work.
- [x] Specify idle model unloading, grace periods, pinning, and active-stream protection for the CPU-first runtime slice; grace-period retention and accelerator preemption remain future work.
- [ ] Specify GPU scheduler priorities, fairness, concurrency, placement, and preemption limits.
- [ ] Define hardware health monitoring and device quarantine behavior.
- [ ] Define authentication and authorization principals, model permissions, device permissions, and administrative actions.
- [ ] Define resource quotas, context limits, request size limits, and rate limiting.
- [ ] Define crash detection, backend restart, failed-run classification, and recovery observability.
- [ ] Define optional TCP endpoint, network exposure policy, TLS, certificate rotation, and client compatibility.
- [ ] Define administrative enable/disable semantics for backend adapters.

## v0.3 — Compatibility and extensibility

- [ ] Define the dynamic CLI capability framework: native command pass-through,
  runtime capability registration, routing, completion, collision handling, and
  evidence integration. See WP-0037.
- [ ] Define OpenAI-compatible chat/completions translation, streaming mapping, errors, and capability gaps.
- [ ] Evaluate and specify a second adapter from vLLM, ONNX Runtime, OpenVINO, or TensorRT-LLM.
- [ ] Define Reference local runner and Docker Model Runner adapter boundaries.
- [ ] Define remote OpenAI-compatible provider policy, credential isolation, endpoint health, and cost attribution.
- [ ] Define plugin discovery, manifest, API compatibility, signing, trust, isolation, and revocation.
- [ ] Define policy profiles for shell, desktop, background services, local applications, and OIP clients.
- [ ] Define audit export, metrics scraping, tracing propagation, and retention controls.

## v1.0 — Production readiness

- [ ] Publish stable API versioning and deprecation policy.
- [ ] Complete threat model, security review, and adversarial validation.
- [ ] Define multi-user and tenant isolation guarantees.
- [ ] Define upgrade, cache migration, rollback, and disaster recovery procedures.
- [ ] Publish backend support matrix, hardware support matrix, capacity guidance, and SLOs.
- [ ] Define OIP integration contracts and interoperability test suite.
