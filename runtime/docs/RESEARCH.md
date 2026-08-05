# Future Research Topics

These topics require investigation before their corresponding backlog items are committed to a stable design.

## Hardware and scheduling

- Normalize heterogeneous GPU and NPU capabilities without losing vendor-specific constraints.
- Compare device memory reporting, unified memory, NUMA, and multi-device placement models.
- Determine whether scheduling should be request-level, model-level, or token-budget based.
- Study safe preemption and batching behavior across supported backends.

## Models and supply chain

- Compare manifest formats and compatibility with existing model registries.
- Establish signing, provenance, license metadata, revocation, and offline verification policy.
- Evaluate cache deduplication, resumable downloads, eviction, and encrypted-at-rest options.
- Define how quantization variants are selected and how quality constraints are expressed.

## Backend interoperability

- Identify the smallest common capability set across local, containerized, and remote engines.
- Measure semantic differences in cancellation, token usage, context limits, tool calls, and streaming.
- Determine process isolation and ABI compatibility requirements for plugins and adapters.
- Evaluate OpenAI-compatible translation gaps and extension mechanisms.

## Security and operations

- Threat-model untrusted model artifacts, malicious prompts, compromised adapters, and remote providers.
- Determine least-privilege device access for Unix services and containerized backends.
- Define audit retention, privacy redaction, secret handling, and tenant isolation.
- Establish crash recovery guarantees and behavior during device loss or backend upgrade.

## OIP integration

- Define discovery, identity, policy, telemetry, and workload handoff boundaries for future OIP integration.
- Determine which controls remain local and which may be delegated to an enterprise control plane.
- Define offline behavior and fail-closed policy when enterprise services are unreachable.

