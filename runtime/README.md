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

The console consumes `RuntimeService` from `MarinaRuntime`. `Runtime::start` is
the single production composition root; it validates configuration and creates
the service with the backend manager, model registry, hardware service, and
event bus. Test-only doubles remain available where individual contracts need
deterministic isolation.

## Phase 5 services

`BackendManager` owns registration, enablement, selection, and health for
backend adapters. `ModelRegistry` stores model metadata only and can persist it
to a registry file. `HardwareService` reports portable system facts and leaves
GPU/NPU discovery as explicit placeholders. All lifecycle changes flow through
the shared `EventBus`.

The runtime registers `LlamaCppAdapter` at startup. If `LLAMA_CPP_LIB_DIR` is
not configured, it remains registered and reports `Unavailable`; there is no
silent mock fallback. Configured builds link directly to `libllama` and never
launch `llama-cli`.

The runtime recursively discovers `.gguf` files from configured local model
roots, including common Hugging Face/Python and Ollama cache directories.
`ModelAcquirer` imports local paths, `file://` URLs, or explicit HTTP(S) URLs
into the managed store with atomic writes and optional SHA-256 verification.
The registry persists when Marina is started through `config::load()`.

The current runner supports one-model-at-a-time loading/unloading,
backend-reported model memory, native tokenizer routing, text generation,
streaming, and cancellation. Chat, embeddings, and tool execution remain
separate capabilities.

Generation is represented by `GenerationRequest`, `GenerationOptions`,
`GenerationStream`, `GenerationResult`, and `GenerationStatistics`. A runtime
client receives the stream immediately; a worker publishes token events while
the client consumes output. Cancellation is an atomic request signal and does
not stop the process or unload the model.
