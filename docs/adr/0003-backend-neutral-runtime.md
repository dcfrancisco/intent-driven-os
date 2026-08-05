# ADR-0003: Backend-Neutral Runtime

- Status: Accepted

## Context

Inference engines differ in model formats, device support, scheduling, streaming behavior, and operational APIs. Coupling callers to one engine would make model management and future deployments brittle.

## Decision

The Intelligent Runtime owns orchestration: registry resolution, policy checks, hardware-aware placement, memory admission, lifecycle, streaming, health, and observability. Inference engines are replaceable adapters behind a common capability-based interface. The initial adapter is `llama.cpp`.

The Phase 3 `Backend` contract covers initialization, shutdown, health, model
visibility, model load/unload, streaming, cancellation, embeddings, and tool
support. The runtime owns `BackendManager`; adapters never become public client
dependencies.

Future adapter targets are vLLM, ONNX Runtime, OpenVINO, TensorRT-LLM, Ollama, and Docker Model Runner.

## Consequences

Clients receive one stable runtime contract and can benefit from backend evolution. The runtime must model capability gaps explicitly and cannot assume every backend supports identical features. Adapter testing and version compatibility become first-class work.

## Alternatives Considered

- Making `llama.cpp` the public API would optimize the first backend at the cost of portability.
- Letting each client integrate engines directly would duplicate policy, lifecycle, and security logic.
- Requiring every backend to expose identical internals would produce an artificial lowest-common-denominator design.

## Future Considerations

Capability negotiation, backend process isolation, plugin signing, remote-provider semantics, and backend-specific performance profiles must be defined before adding adapters.
