# Intelligent Runtime Backlog

Items are intentionally written as design and implementation slices for future work. Nothing in this backlog is implemented by this documentation module.

## v0.1 — Contracts and local foundation

### Registry and models

- [ ] Define stable model identity, version, source, license, capabilities, and ownership fields.
- [ ] Define the model manifest schema, including artifact URLs, SHA256 digests, format, context limits, quantization variants, and backend compatibility.
- [ ] Define registry states: discovered, trusted, cached, unavailable, revoked, and deprecated.
- [ ] Define model cache layout, atomic writes, eviction metadata, and cache corruption handling.
- [ ] Define SHA256 verification and the trust decision recorded for every artifact.

### Hardware and resources

- [ ] Define CPU, GPU, and NPU discovery records and capability normalization.
- [ ] Define memory budgets, reservations, accounting units, and pressure signals.
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

- [ ] Specify memory manager behavior under admission failure and memory pressure.
- [ ] Specify idle model unloading, grace periods, pinning, and active-stream protection.
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
- [ ] Define Ollama and Docker Model Runner adapter boundaries.
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
