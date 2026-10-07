# Intelligent Runtime Architecture

## Architecture overview

The runtime is a control plane that turns model requests into policy-checked, resource-aware backend sessions. Callers submit intent through a stable API; they do not select backend-specific objects or manage device memory directly.

```mermaid
flowchart LR
    Client[Shell / Desktop / Service / App] --> API[Runtime API]
    API --> Auth[Authentication & Authorization]
    Auth --> Orchestrator[Runtime Orchestrator]
    Orchestrator --> Registry[Model Registry & Manifest Store]
    Orchestrator --> Policy[Policy & Quota Manager]
    Orchestrator --> Hardware[Hardware Abstraction]
    Orchestrator --> Memory[Memory Manager]
    Orchestrator --> Scheduler[Device / GPU Scheduler]
    Scheduler --> Adapter[Backend Adapter Interface]
    Adapter --> Llama[llama.cpp]
    Adapter -. future .-> Other[vLLM / ONNX / OpenVINO / TensorRT-LLM / Remote]
    Orchestrator --> Observe[Health / Metrics / Tracing / Audit]
    Registry --> Cache[Model Cache]
```

The runtime should be usable in-process for local consumers and as a local service over a Unix socket. An optional TCP endpoint may be added for controlled multi-process or enterprise deployments.

## Marina product architecture

Marina is not only a llama.cpp wrapper. It is a standalone model service with
separate control, artifact, inference, compatibility, routing, and integration
planes. Each plane has its own contract and failure boundary.

```mermaid
flowchart TB
    subgraph Clients[Clients]
        OID[OID Console and system]
        Assistants[AI assistants and applications]
        SDK[Python / JavaScript / OpenAI SDK clients]
    end

    subgraph Edge[Marina API edge]
        Native[Versioned native API]
        OpenAI[OpenAI-compatible API]
        Ollama[Ollama-compatible API]
        Anthropic[Anthropic-compatible API]
        MCP[MCP / agent control surface]
    end

    subgraph Service[Marina standalone service]
        Auth[Identity, authorization, quotas]
        Sessions[Chat sessions and request state]
        Router[Capability router and policy admission]
        Scheduler[Memory/device scheduler]
        Lifecycle[Model lifecycle and service supervisor]
        Evidence[Metrics, traces, audit, routing explanations]
    end

    subgraph Resources[Model resources]
        Catalog[Model catalog and manifests]
        Store[Verified model store]
        Providers[Local and remote providers]
        Adapters[Backend adapters]
    end

    OID --> Native
    Assistants --> OpenAI
    SDK --> OpenAI
    Assistants --> Ollama
    Assistants --> Anthropic
    Assistants --> MCP
    Native --> Auth
    OpenAI --> Auth
    Ollama --> Auth
    Anthropic --> Auth
    MCP --> Auth
    Auth --> Sessions
    Sessions --> Router
    Router --> Scheduler
    Router --> Lifecycle
    Scheduler --> Adapters
    Lifecycle --> Catalog
    Lifecycle --> Store
    Router --> Providers
    Adapters --> Providers
    Service -. telemetry .-> Evidence
    Router -. decision .-> Evidence
```

### Capability matrix

| Plane | Ollama-like capability | Docker Model Runner-like capability | OmniRoute-like capability | Marina status |
| --- | --- | --- | --- | --- |
| Model lifecycle | pull, list, show, copy, delete, load/unload, version | model pull/package/list/inspect and on-demand load | provider/model aliases | Local GGUF pull/list/load exists; full lifecycle planned |
| Inference | generate, chat, streaming, keep-alive | OpenAI/Ollama/Anthropic-compatible inference and embeddings | one endpoint across providers | Local streaming exists; HTTP compatibility planned |
| Model features | tools, structured output, images, embeddings, templates/parameters | embeddings, engine selection, image generation where supported | capability-aware model/provider selection | Capability contract exists; features are staged |
| Routing | local model selection | engine/model selection | scoring, fallback, retries, quota, circuit breakers, cost/latency policy | Router contract planned; not yet production-complete |
| Distribution | model library and local cache | OCI/Hugging Face packaging | provider credential/account management | GGUF local/HTTP import exists; OCI/provider management planned |
| Agent integration | tool calls | assistant/IDE compatibility | MCP/A2A gateway and route control | Governed assistant boundary planned |
| Operations | local daemon | Docker-managed service | gateway service | Marina daemon exists; cross-platform service installers planned |

The matrix is a compatibility target, not a claim that every capability is
implemented. Unsupported capabilities must return explicit capability errors;
the runtime must never silently emulate tools, structured output, embeddings,
or routing behavior it cannot enforce.

### Service installation and lifecycle

The runner is installable independently of OID and can run as a user or system
service. The service manager is an operating-system adapter, not part of the
inference backend:

| OS | User service | System service | Default local transport |
| --- | --- | --- | --- |
| Linux | systemd user unit | systemd unit | Unix socket |
| macOS | launchd LaunchAgent | launchd LaunchDaemon | Unix socket |
| Windows | per-user service/task | Windows Service | named pipe |

`marina service install|start|stop|restart|status|uninstall` must be
idempotent, preserve model data during upgrades, and require explicit elevated
privilege only for system-wide installation. A service restart must not be
treated as a model failure; clients receive a recoverable transport/lifecycle
error and can retry with their request identity.

### API surfaces

The native API is the source of truth and covers model/catalog operations,
chat and completion, embeddings, structured output, tool-call proposals,
streaming, cancellation, health, hardware, routing explanations, metrics, and
service lifecycle. Compatibility endpoints translate into this API:

- OpenAI-compatible `/v1/models`, `/v1/chat/completions`,
  `/v1/completions`, and `/v1/embeddings`.
- Ollama-compatible `/api/generate`, `/api/chat`, `/api/tags`, `/api/show`,
  `/api/pull`, `/api/ps`, and model lifecycle endpoints where semantics map
  cleanly.
- Anthropic-compatible messages are an optional compatibility edge after the
  OpenAI/Ollama surfaces are stable.
- MCP exposes bounded management and assistant capabilities; it is not a
  bypass around authorization or evidence.

Every request carries a client identity, request ID, model/provider policy,
capability requirements, resource limits, and cancellation correlation. Every
response identifies the selected model/backend, compatibility translation,
usage metrics, and terminal outcome.

## Phase 5 runtime services

The runtime foundation now separates four service concerns:

- `BackendManager` owns backend registration, enable/disable state, active selection, and health.
- `ModelRegistry` owns persistent model metadata; it does not download, parse, load, or infer models.
- `HardwareService` owns normalized CPU, memory, operating-system, architecture, and SIMD discovery, with GPU/NPU placeholders.
- `RuntimeService` is the client-facing read/query boundary consumed by the AI Console.

The runtime owns the composition root and registers `LlamaCppAdapter` through
the common `Backend` contract. The `oid-llama-cpp-adapter` package re-exports
that stable runtime boundary for future plugin loading; the console never
constructs or accesses a concrete adapter. The `oid-llama-cpp-sys` crate is the
only unsafe boundary and links directly to `libllama` when configured.

### Native API boundary

The adapter calls `llama_backend_init`, `llama_backend_free`,
`llama_print_system_info`, `llama_model_load_from_file`, `llama_model_free`,
`llama_model_size`, and `llama_tokenize`. The public runtime exposes only
descriptors, health, model lifecycle results, and token counts. No llama.cpp
pointer, context, sampler, GGUF parser, or inference operation crosses the
runtime API.

### Model discovery and lifecycle

`ModelDiscovery` recursively scans configured directories for case-insensitive
`.gguf` files and registers metadata without opening a model. The canonical
default is `$HOME/.marina/models`; legacy user data, `~/Models`, Hugging
Face/Python caches, ModelScope caches, and Ollama caches remain discoverable
for migration and interoperability. Repository-local `./models` is not an
operational model store.

The initial loader admits one model. It publishes `ModelLoading`, transitions
metadata to `Loading`, calls the active backend, records backend-reported
memory, then publishes `ModelLoaded`. Failure records `Failed` and emits an
error event. Unload follows the corresponding `ModelUnloading` and
`ModelUnloaded` path.

### Inference lifecycle

```mermaid
sequenceDiagram
    participant Client
    participant Runtime
    participant Backend
    participant Bus
    Client->>Runtime: generate(GenerationRequest)
    Runtime-->>Client: GenerationStream
    Runtime->>Bus: GenerationStarted
    Runtime->>Backend: decode(prompt, options)
    Backend->>Bus: FirstToken / TokenGenerated
    Backend-->>Client: token fragments
    Client->>Runtime: cancel()
    Runtime->>Backend: cancellation signal
    Runtime->>Bus: GenerationCancelled or GenerationCompleted
```

The runtime owns request IDs, admission, cancellation, stream delivery, and
metrics. The backend owns context creation, sampling, decoding, and token
piece conversion. The first native adapter uses the official llama.cpp C API;
future adapters implement the same callback boundary.

## Module responsibilities

| Module | Responsibility | Must not own |
| --- | --- | --- |
| API boundary | Versioned request, response, streaming, and error contracts | Backend or device details |
| Orchestrator | Model lifecycle, request coordination, and run state | Tensor operations |
| Model registry | Model identity, manifests, versions, sources, and trust metadata | Download implementation policy decisions |
| Model cache | Verified artifact storage, eviction, and integrity state | Model parsing |
| Hardware abstraction | CPU/GPU/NPU discovery, capabilities, and health | Vendor-specific scheduling policy |
| Memory manager | Reservations, budgets, accounting, and pressure signals | Allocating tensors inside a backend |
| Scheduler | Placement, concurrency, priority, and device admission | Inference mathematics |
| Quantization policy | Selects an allowed model variant for hardware and policy | Quantizing model files |
| Backend adapters | Map common runtime operations to backend capabilities | General runtime policy |
| Security layer | Authentication, authorization, quotas, and isolation decisions | Model quality decisions |
| Observability | Metrics, traces, audit events, and health reports | Mutating runtime state without an authorized command |
| CLI/client | Human-facing command translation and presentation | Owning runtime state |

## Core lifecycle

```mermaid
stateDiagram-v2
    [*] --> Registered
    Registered --> Resolving
    Resolving --> Downloading
    Resolving --> Ready
    Downloading --> Verifying
    Verifying --> Cached
    Cached --> Loading
    Loading --> Loaded
    Loaded --> Running
    Running --> Loaded
    Loaded --> Unloading
    Unloading --> Ready
    Loading --> Failed
    Running --> Failed
    Failed --> Recovering
    Recovering --> Ready
    Recovering --> Failed
```

A model artifact must be verified before it becomes loadable. A run must be admitted only after authorization, quota, context, memory, hardware, and backend capability checks. Idle unloading is an optimization and must never invalidate an active stream.

## Backend boundary

The common adapter contract should cover discovery, capability reporting, load/unload, run admission, cancellation, streaming output, health, and structured errors. Adapters translate runtime concepts into backend-specific calls; they do not expose backend handles through the public API.

Initial adapter: `llama.cpp`.

Planned adapters: vLLM, ONNX Runtime, OpenVINO, TensorRT-LLM, Ollama, Docker Model Runner, and remote OpenAI-compatible providers.

## Security and trust

Model sources, downloaded bytes, manifests, and runtime actions are separately auditable. SHA256 verification is the minimum integrity check for cached artifacts. Authentication identifies the caller; authorization determines which models, devices, capabilities, and resource budgets that caller may use. Remote providers require explicit endpoint and credential policy and must not be treated as local devices.

## Design principles

- Backend neutrality: callers depend on runtime capabilities, not engine APIs.
- Hardware awareness: placement and model variant selection use discovered capabilities and live health.
- Explicit policy: no model load, run, download, or backend enablement bypasses authorization.
- Recoverability: lifecycle transitions, failures, and recovery decisions are observable.
- Composable transport: Unix socket first; optional TCP and TLS later.
- Stable contracts: API versioning and capability negotiation precede backend expansion.
- Minimal core: no tokenizer, parser, tensor, kernel, or inference implementation in this module.
