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
- `adapters/llama-cpp` — public package boundary for the native adapter
- `adapters/llama-cpp/sys` — opt-in direct FFI/link boundary for libllama

The Phase 2 console consumes `RuntimeService` and `MockRuntime`. The event bus
publishes lifecycle and command events without requiring an inference backend.
All backend, inference, Linux-operation, and real hardware behavior remains
future work.

## Phase 4 services

`BackendManager` owns registration, enablement, selection, and health for
backend adapters. `ModelRegistry` stores model metadata only and can persist it
to a registry file. `HardwareService` reports portable system facts and leaves
GPU/NPU discovery as explicit placeholders. All lifecycle changes flow through
the shared `EventBus`.

The runtime registers `LlamaCppAdapter` at startup. If `LLAMA_CPP_LIB_DIR` is
not configured, it remains registered and reports `Unavailable`; there is no
silent mock fallback. Configured builds link directly to `libllama` and never
launch `llama-cli`.

Phase 4 adds recursive `.gguf` discovery, metadata-only registry population,
one-model-at-a-time loading/unloading, backend-reported model memory, and
native tokenizer routing. Text generation, streaming, chat, embeddings, and
GGUF parsing remain outside this milestone.
