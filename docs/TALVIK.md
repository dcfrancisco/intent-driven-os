# Talvik Model Runtime

Talvik is the provisional product identity for the repository's standalone,
local-first model runtime. It is real runtime infrastructure: the service owns
model discovery, import, load/unload, inference, streaming, cancellation,
admission, authentication, health, and recovery behind a backend-neutral API.
OID is a client. Talvik does not bypass OID policy for computer operations.

The public name remains subject to legal, trademark, package-registry, and
domain clearance. That review is deliberately non-blocking and tracked in
WP-0075 and `runtime/BACKLOG.md`.

## Current product boundary

The qualified CPU-first slice supports multiple resident GGUF models, one
active generation, streaming, cancellation, conditional embeddings, and
OpenAI-compatible chat/completions translation through the llama.cpp adapter,
including tool calls for GGUF templates that advertise tool support. It is not
a model scheduler with concurrent generations, an agent loop, a training
system, or an unrestricted network gateway. Tool calls remain
capability-gated and are rejected explicitly for unsupported templates.

The OpenAI edge is a limited text inference subset (Chat Completions, partial
legacy Completions, text-only Responses), not full OpenAI API compatibility.
See [Talvik OpenAI API Compatibility](./TALVIK-OPENAI-COMPATIBILITY.md) for
supported request fields, response behavior, unsupported features, and
qualification evidence. Broader inference-surface compatibility is tracked by
[ADR-0031](./adr/0031-openai-inference-api-compatibility.md) and
[WP-0108](./work-packages/WP-0108-openai-inference-api-compatibility.md).

`talvikd` is the daemon and `talvik` is its client. The former `marina` and
`marinactl` binaries are no longer built or packaged. Existing `$HOME/.marina`
data remains the canonical store and is not deleted or moved.

## Quick start

```sh
./scripts/install-model-runner.sh --native-lib-dir "$HOME/.marina/lib"
export PATH="$HOME/.marina/bin:$PATH"
talvikd
```

In another terminal:

```sh
talvik status
talvik models
talvik load qwen2.5-0.5b-instruct-q4_k_m
talvik run qwen2.5-0.5b-instruct-q4_k_m "What is the capital of France?"
talvik unload qwen2.5-0.5b-instruct-q4_k_m
```

Import an artifact with an expected SHA-256 when available:

```sh
talvik import <source-url-or-path> <model-id> <sha256>
```

## Optional ordered fallback

Routing is disabled by default. Direct `talvik run <model> <prompt>` remains
the primary, explicit path. To opt into the bounded first routing slice:

```sh
export TALVIK_ROUTING_ENABLED=1
export TALVIK_ROUTE_MODELS=preferred-model,fallback-model
talvik run --route "What is the capital of France?"
```

The client reports each selection and unavailable candidate. Fallback occurs
only before output begins; once any token is delivered, failure is returned
instead of silently switching models. This initial policy routes among model
IDs served by the local runtime. Capability-aware backend/provider routing,
durable decision evidence, remote credentials, and circuit breaking remain
WP-0060 work.

## Configuration and migration

Talvik uses the following configuration variables:

| Variable | Purpose |
| --- | --- |
| `TALVIK_HOME` | Complete Talvik home directory |
| `TALVIK_CONFIG` | Configuration file |
| `TALVIK_SOCKET` | Local socket |
| `TALVIK_MODEL_DIR` | Model store |
| `TALVIK_MODEL_DIRECTORY` | Canonical alias for the model store |
| `TALVIK_REGISTRY` | Registry file |
| `TALVIK_STATE_DIR` | Runtime state |
| `TALVIK_DEFAULT_MODEL` | Model Talvik loads at daemon startup; defaults to the pinned Qwen 0.5B GGUF |
| `TALVIK_DEFAULT_MODEL_SOURCE` | Source URL/path for acquiring a missing custom default model |
| `TALVIK_DEFAULT_MODEL_SHA256` | Expected SHA-256 for the default model download |
| `TALVIK_AUTO_BOOTSTRAP` | Enable default-model bootstrap; defaults to true |
| `TALVIK_AUTO_DOWNLOAD` | Permit download of a missing default model; defaults to true |
| `TALVIK_AUTO_LOAD` | Load the configured default model at startup; defaults to true |
| `TALVIK_TOKEN_FILE` | Token database |
| `TALVIK_HTTP_ADDR` | Optional HTTP listener |
| `TALVIK_HTTP_AUTH` | HTTP authentication control |

Memory scheduling is bounded and CPU-first: one generation runs at a time,
idle unpinned residents may be evicted by LRU under pressure, and registered
model metadata remains available for reload. The policy can be controlled with
`TALVIK_MAX_RESIDENT_MODELS`, `TALVIK_MEMORY_BUDGET_BYTES`,
`TALVIK_SAFETY_MARGIN_BYTES`, `TALVIK_AUTOMATIC_EVICTION`,
`TALVIK_RECOVERY_BUDGET`, `TALVIK_ADMISSION_TIMEOUT_SECONDS`,
`TALVIK_IDLE_RETENTION_SECONDS`, `TALVIK_QUEUE_CAPACITY`, and
`TALVIK_PINNED_MODELS`. Defaults preserve multi-model residency where the
detected budget permits it; explicit context sizes are not silently reduced.

For a container deployment, use `docker/talvik/compose.yaml`. It keeps the
host port on loopback by default, requires bearer authentication for HTTP API
requests, and separates model and state volumes.

The authenticated HTTP surface also provides a partial Ollama-compatible
local model-runner API: `/api/chat`, `/api/generate`, `/api/embed`,
`/api/embeddings`, `/api/show`, `/api/tags`, `/api/ps`, and `/api/version`.
Text inference uses NDJSON streaming by default. Registry distribution,
Modelfile creation, model copy/delete, per-request keep-alive, and multimodal
support remain incomplete. See
[Talvik Ollama API Compatibility](./TALVIK-OLLAMA-COMPATIBILITY.md),
[ADR-0032](./adr/0032-talvik-ollama-model-runner-api.md), and
[WP-0109](./work-packages/WP-0109-talvik-ollama-model-runner-api.md).
The `/v1/*` OpenAI-compatible edge is separate; its current status is documented
in [Talvik OpenAI API Compatibility](./TALVIK-OPENAI-COMPATIBILITY.md).

Defaults resolve under `$HOME/.marina`. The `MARINA_*` environment variables
remain compatibility aliases, while `TALVIK_*` variables take precedence.
Talvik does not delete or silently migrate existing data.

At startup, `talvikd` loads `qwen2.5-0.5b-instruct-q4_k_m` by default. If it is
not registered, Talvik acquires the pinned GGUF artifact from Hugging Face and
verifies its SHA-256 before loading. A pull or load failure is reported as a
warning and the daemon continues. Set `default_model: none` or
`TALVIK_DEFAULT_MODEL=none` to disable automatic loading. For another default
model, configure its source and optional checksum as well.

## Security defaults

Local Unix sockets are user-only. HTTP is disabled unless configured, bearer
authentication is enabled by default, and non-loopback binding requires an
explicit override. Routing cannot enable itself and does not weaken model or
transport admission controls.
