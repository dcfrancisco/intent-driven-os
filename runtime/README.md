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

## IDE access today

The current usable IDE path is a loopback OpenAI-compatible bridge:

```sh
marinactl auth token create ide
python3 scripts/marina-openai-proxy.py
```

Configure the IDE with base URL `http://127.0.0.1:11435/v1`, the printed value
as its Bearer/API key, and a model returned by `GET /v1/models`. The bridge
supports both non-streaming and server-sent-event (`stream: true`) text
responses. The token file is `$HOME/.marina/tokens` by default and is created
with user-only permissions.
Set `MARINA_TOKEN_FILE` to use another secret-store integration point.

Persistent runner settings belong in `$HOME/.marina/config.yaml` on Unix or
`%USERPROFILE%\.marina\config.yaml` on Windows. Environment variables override
file values for tests and deployment automation.

This bridge currently supports model listing and non-streaming text chat. The
token is an access credential for Marina; it is not a provider API key. Remote
BYOK provider credentials are not yet implemented and must remain in the IDE
or an OS secret manager. The bridge binds to loopback by default and must not
be exposed publicly without the planned authenticated service boundary.
