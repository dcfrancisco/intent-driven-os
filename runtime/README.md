# Runtime Crate

Phase 1 runtime crate boundaries:

- `core` — orchestration and public runtime handle
- `config` — deterministic configuration loading boundary
- `logging` — logging initialization boundary
- `lifecycle` — explicit startup/shutdown state machine
- `hardware` — CPU/GPU/NPU discovery interface
- `backends` — replaceable inference adapter interface
- `models` — model registry interface
- `security` — principal and policy interfaces
- `api` — transport-neutral runtime API
- `adapters/llama-cpp` — contract-only adapter skeleton; no inference

The Phase 2 console consumes `RuntimeService` and `MockRuntime`. The event bus
publishes lifecycle and command events without requiring an inference backend.
All backend, inference, Linux-operation, and real hardware behavior remains
future work.

## Phase 3 services

`BackendManager` owns registration, enablement, selection, and health for
backend adapters. `ModelRegistry` stores model metadata only and can persist it
to a registry file. `HardwareService` reports portable system facts and leaves
GPU/NPU discovery as explicit placeholders. All lifecycle changes flow through
the shared `EventBus`.
