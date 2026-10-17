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

The qualified CPU-first slice supports one loaded GGUF model and one active
generation through the llama.cpp adapter. It is not a multi-model scheduler,
agent loop, model-training system, or unrestricted network gateway.

`talvik` is the primary command. `marina` and `marinactl` remain compatibility
binaries during migration, and existing `$HOME/.marina` installations are
used in place. No model, token, registry, or configuration data is moved or
deleted.

## Quick start

```sh
./scripts/install-model-runner.sh --native-lib-dir "$HOME/.marina/lib"
export PATH="$HOME/.marina/bin:$PATH"
talvik serve
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

Talvik-prefixed variables take precedence over their Marina compatibility
aliases:

| Primary | Compatibility alias |
| --- | --- |
| `TALVIK_HOME` | `MARINA_HOME` |
| `TALVIK_CONFIG` | `MARINA_CONFIG` |
| `TALVIK_SOCKET` | `MARINA_SOCKET` |
| `TALVIK_MODEL_DIR` | `MARINA_MODEL_DIR` |
| `TALVIK_REGISTRY` | `MARINA_REGISTRY` |
| `TALVIK_STATE_DIR` | `MARINA_STATE_DIR` |
| `TALVIK_TOKEN_FILE` | `MARINA_TOKEN_FILE` |
| `TALVIK_HTTP_ADDR` | `MARINA_HTTP_ADDR` |
| `TALVIK_HTTP_AUTH` | `MARINA_HTTP_AUTH` |

Defaults continue to resolve under `$HOME/.marina` for compatibility. A future
store rename requires an explicit migration, rollback plan, and release gate;
it must not be performed as an incidental rebrand.

## Security defaults

Local Unix sockets are user-only. HTTP is disabled unless configured, bearer
authentication is enabled by default, and non-loopback binding requires an
explicit override. Routing cannot enable itself and does not weaken model or
transport admission controls.
