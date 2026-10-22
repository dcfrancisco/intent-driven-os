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
active generation, streaming, cancellation, embeddings, and OpenAI-compatible
chat/completions translation through the llama.cpp adapter. It is not a model
scheduler with concurrent generations, an agent loop, a training system, or an
unrestricted network gateway. Tool calls are capability-reported and rejected
explicitly until a backend implements them.

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
| `TALVIK_REGISTRY` | Registry file |
| `TALVIK_STATE_DIR` | Runtime state |
| `TALVIK_TOKEN_FILE` | Token database |
| `TALVIK_HTTP_ADDR` | Optional HTTP listener |
| `TALVIK_HTTP_AUTH` | HTTP authentication control |

Defaults resolve under `$HOME/.marina`. The `MARINA_*` environment variables
remain compatibility aliases, while `TALVIK_*` variables take precedence.
Talvik does not delete or silently migrate existing data.

## Security defaults

Local Unix sockets are user-only. HTTP is disabled unless configured, bearer
authentication is enabled by default, and non-loopback binding requires an
explicit override. Routing cannot enable itself and does not weaken model or
transport admission controls.
