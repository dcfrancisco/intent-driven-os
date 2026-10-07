# ADR-0013: Cross-Platform Standalone Runner and Assistant API

- Status: Accepted
- Date: 2026-10-07
- Related: ADR-0003, ADR-0004, ADR-0006, ADR-0010, ADR-0011, ADR-0012, ADR-0014, ADR-0015; WP-0065 through WP-0071

## Decision

Marina is a standalone model-runner product. It must be installable and
operable without the OID interactive console, while OID remains a first-class
client and system integration of the same runtime service.

The product has one backend-neutral, versioned API with multiple transports:

```text
OID Console / OID system / AI assistant / application
                         |
       native local API + OpenAI-compatible API
                         |
                 Marina runtime service
                         |
              backend router and adapters
```

The initial local transport is Unix-socket based on Linux and macOS. The first
Windows release uses loopback TCP through the same transport-neutral protocol;
named-pipe transport remains the hardening target. A loopback HTTP transport
is the assistant and application integration surface; an OpenAI-compatible
edge may translate chat/completions requests into the internal API without
exposing backend types.
Network exposure beyond loopback requires explicit configuration, identity,
authorization, and transport security.

The runner is distributed independently for Linux, macOS, and Windows. The
default installation is user-scoped; system package formats may place binaries
in system paths. The logical Marina home is:

```text
Linux/macOS: $HOME/.marina/{bin,lib,models}
Windows:     %USERPROFILE%\\.marina\\{bin,lib,models}
```

Every release artifact contains the native backend payload for its target
platform under `lib/`. An archive containing only `marina` and `marinactl` is
not a runnable local-inference release. Windows packages may duplicate DLLs
into `bin/` for the OS loader while preserving `lib/` as the canonical managed
library store.

Models are always managed by the runner in its Marina model store. They are
not checked into or downloaded into the source repository. OID uses the API
and does not own model files, backend handles, or provider credentials. When
OID and Marina run on the same machine, Marina is the single model authority:
the OID console, OID system services, and external IDE clients see the same
model inventory, loaded-model state, health, and request metrics through the
local service boundary. They do not maintain parallel registries or copy
models into OID-owned directories.

## Boundaries

- Marina owns model discovery, acquisition, manifests, loading, inference,
  streaming, cancellation, health, routing, and runtime evidence.
- OID owns its console and governed system operations; its local system client
  calls Marina for model discovery, load/admission, and assistant inference.
  OID policy remains authoritative for OID actions, while Marina policy is
  authoritative for model/resource access.
- AI assistants and applications receive a stable request/stream/tool-facing
  API and cannot access native backend pointers or policy internals.
- Model providers and backend adapters remain replaceable implementation
  details behind the Marina API.

## Delivery sequence

1. Cross-platform user/system installer artifacts and lifecycle commands.
2. Portable service transport and health/model/generation API.
3. OpenAI-compatible assistant endpoint with streaming and error mapping.
4. OID client integration, assistant session/tool policy, and end-to-end
   interoperability tests.

This decision describes the product direction; Windows transport, HTTP API,
packaging, multi-model scheduling, assistant tools, Linux distro integration,
and the complete capability contract are not yet complete.

## Same-machine access rule

Same-machine access is an API relationship, not permission to read Marina's
model directory. Development may point both processes at an explicitly shared
test directory, but product operation uses a local authenticated transport.
The production design provides separate local principals for the interactive
user, OID system client, and other IDE/application clients, with model access
and resource limits recorded in the Marina audit stream.
