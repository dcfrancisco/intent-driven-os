# Talvik/OID Model Runner Gap Review

Date: 2026-11-01

## Scope

This is a documentation-only review. No source code was changed.

The target question is: **what is still missing, and what is currently wrong,
before this project can be used as a dependable local model runner with an
Ollama-like user experience?**

The project at
`/Users/dannyfrancisco/codes/poc-projects/intent-driven-os` was compared with
the local Ollama checkout at `/Users/dannyfrancisco/codes/ollama`. Ollama is
used only as a behavior and feature reference; this review does not recommend
copying its internal architecture.

Evidence reviewed:

- [Project operating brief](../PROJECT-OPERATING-BRIEF.md)
- [Runtime README](../../runtime/README.md)
- [Runtime API contract](../TALVIK-API-CONTRACT.md)
- [Ollama compatibility matrix](../TALVIK-OLLAMA-COMPATIBILITY.md)
- [Runtime source API](../../runtime/src/api.rs)
- [Runtime service](../../runtime/src/service.rs)
- [HTTP bridge](../../console/src/bin/http.rs)
- [Model registry](../../runtime/src/models.rs)
- [Native llama.cpp adapter](../../runtime/src/native_llama.rs)
- [Model runner contract](../../crates/model-runner/src/lib.rs)
- Ollama route registration at `/Users/dannyfrancisco/codes/ollama/server/routes.go`
- Ollama API behavior reference at `/Users/dannyfrancisco/codes/ollama/docs/api.md`
- Ollama OpenAPI reference at `/Users/dannyfrancisco/codes/ollama/docs/openapi.yaml`

## Executive verdict

The project is **not a non-working prototype**. It has a real llama.cpp-backed
GGUF loading and generation path, streaming, cancellation, model residency,
resource admission, embeddings, structured output, and an HTTP compatibility
edge.

It is also **not yet an Ollama-equivalent local product**. The current state is
better described as:

> a security-conscious, CPU-first GGUF runtime with a partial Ollama-shaped
> HTTP compatibility layer and a stronger internal lifecycle model.

The largest gap is not basic text generation. The largest gap is the complete
model lifecycle and operational product around generation:

1. A user cannot manage models with the normal Ollama lifecycle (`pull`,
   `create`, `copy`, `delete`, blob handling, aliases/tags, registry manifests).
2. The model metadata contract is too thin for reliable discovery, selection,
   and compatibility.
3. Hardware support and cross-platform qualification are incomplete.
4. The service has explicit placeholders and qualification gaps that prevent a
   clean “ready” claim.
5. The compatibility edge intentionally rejects or changes behavior that common
   Ollama clients expect.

## Priority summary

| Priority | Gap | Why it matters |
|---|---|---|
| P0 | Native dependency and clean-machine bootstrap | Without a correctly built native library, the daemon starts but inference is unavailable. A user needs a deterministic install and first-run path. |
| P0 | End-to-end lifecycle qualification | Streaming, cancellation, disconnect recovery, queueing, unload/reload, and duplicate-daemon behavior must pass on release artifacts, not only unit tests. |
| P0 | Cross-platform native qualification | Linux is the strongest path; macOS arm64 and Windows remain qualification dependencies, while GPU behavior is not generally proven. |
| P0 | Model acquisition and identity | Raw GGUF import is not equivalent to Ollama model names, tags, manifests, aliases, trusted registries, and resumable distribution semantics. |
| P1 | Ollama request semantics | `keep_alive`, raw/context reuse, suffix, custom templates, thinking controls, and broader options are missing or intentionally rejected. |
| P1 | Model inspection fidelity | `/api/show` lacks Modelfile, template, license, parameters, and raw GGUF metadata. |
| P1 | Residency fidelity | `/api/ps` lacks Ollama-style expiry behavior and complete device-memory reporting. |
| P1 | Hardware acceleration | GPU/NPU discovery is still partly placeholder-based; actual backend selection, memory accounting, and fallback require broader qualification. |
| P1 | Security policy integration | HTTP token scopes exist, but the runtime policy boundary itself is explicitly permissive and not yet the governing policy engine. |
| P2 | Broader modality/API surface | Audio, speech, video, Anthropic compatibility, web tools, and cloud features are outside the local-runner target and should not be implied by Ollama parity. |

## What is already working

The following capabilities are present in code and are materially beyond a
mock:

- GGUF discovery and local/URL acquisition.
- Persistent model metadata registration.
- Model load/unload and multiple resident model bookkeeping.
- Native tokenization and streaming generation.
- Request cancellation and bounded queueing.
- Resource admission and automatic eviction policy.
- Persistent native context reuse and explicit context reset in the runner
  contract.
- Sampling controls, stop sequences, structured output, and selected tool
  handling.
- Embeddings through capable backends.
- Ollama-shaped `/api/version`, `/api/tags`, `/api/ps`, `/api/show`,
  `/api/chat`, `/api/generate`, `/api/embed`, and legacy
  `/api/embeddings` routes.
- OpenAI-shaped chat/completions/responses/embeddings routes.

These are useful foundations, but route presence must not be confused with
behavioral parity. The compatibility document itself classifies most of these
routes as partial.

## What is wrong or risky in the current project

### 1. “Ollama-like” is not currently defined as a complete acceptance target

The project has an explicit subset, but the user-facing goal can easily be read
as full Ollama compatibility. Ollama currently registers the following local
API families in
`/Users/dannyfrancisco/codes/ollama/server/routes.go`: version, status, pull,
push, tags, show, delete, create, blobs, copy, experimental web operations,
ps, generate, chat, embed, and embeddings. It also exposes a larger OpenAI
surface and audio routes.

Talvik implements only a subset and returns explicit unsupported responses for
several common lifecycle operations. That is a valid product choice, but the
product should be described as a **Talvik runtime with a partial Ollama
compatibility edge**, not as an Ollama replacement.

### 2. Model identity is artifact-oriented rather than user-oriented

The registry is keyed by a stable local ID and stores a filesystem location,
family, quantization, capabilities, and estimates
([models.rs](../../runtime/src/models.rs)). Ollama users expect names such as
`model:tag`, defaulting to `latest`, with aliases and version-specific
artifacts.

Consequences:

- A model downloaded under two names can become two registry entries.
- There is no complete manifest/digest identity model.
- There is no clear alias/tag resolution contract.
- There is no safe copy/rename/delete lifecycle.
- A model's source, license, trust state, and ownership are not first-class
  enough for distribution and cleanup.

Raw local import and URL acquisition are useful, but they do not provide the
model catalog behavior users associate with Ollama.

### 3. Model management is incomplete

Compared with Ollama's API reference, the following are missing:

- `POST /api/pull` as a registry-manifest/progress operation.
- `POST /api/push`.
- `POST /api/create` for Modelfile/template-based model definitions.
- `POST /api/copy`.
- `DELETE /api/delete`.
- `HEAD /api/blobs/{digest}` and `POST /api/blobs/{digest}`.
- Registry authentication, trust policy, manifest resolution, blob
  deduplication, and garbage collection.

The project has an internal `model_pull` operation, but it is a raw local path
or URL import contract. It should not be presented as an implementation of
Ollama pull.

### 4. Model metadata is insufficient for predictable prompting

Ollama's `/api/show` can expose model details used by clients and users:
template, parameters, license, Modelfile, capabilities, and model metadata.
Talvik's compatibility response is deliberately thinner and may return an
empty `model_info` object when raw metadata is unavailable
([TALVIK-OLLAMA-COMPATIBILITY.md](../TALVIK-OLLAMA-COMPATIBILITY.md)).

This affects:

- automatic context-size and capability selection;
- reproducibility of prompts;
- debugging model-specific chat-template behavior;
- deciding whether a model supports vision, tools, embeddings, or thinking;
- displaying provenance and license information;
- diagnosing why two GGUF files behave differently.

The registry's declared metadata must be distinguished from metadata extracted
and verified from the GGUF artifact.

### 5. Request semantics differ in common, observable ways

The Ollama API reference documents these generation inputs:

- `keep_alive`;
- `raw`;
- deprecated `context` reuse;
- `suffix`;
- custom `template`;
- `think`;
- a wider set of model options;
- model names with tags.

Talvik currently rejects or does not implement several of them, as recorded in
the compatibility matrix. This means an existing Ollama client can connect and
still fail on ordinary workflows such as:

- keeping a model warm between requests;
- fill-in-the-middle completion;
- supplying a custom prompt template;
- reusing a returned context;
- using model-specific advanced options;
- enabling model-supported thinking behavior through the normal API.

The current default of one active generation is a reasonable safety boundary,
but it should be documented as a deliberate service semantic rather than
implicitly treated as Ollama scheduling behavior.

### 6. Residency and keep-alive semantics are not equivalent

Talvik has runtime-owned residency, idle retention, admission, and eviction
policy. Ollama exposes loaded-model state with an expiration model, including
`expires_at`-style behavior in running-model responses.

The current `/api/ps` compatibility response does not provide complete
expiration semantics or complete VRAM accounting. As a result:

- clients cannot reliably predict whether the next request will reload;
- users cannot control warm residency per request;
- the reported state may not explain a memory-pressure eviction;
- device placement and host/device memory are not always comparable to Ollama
  output.

### 7. Hardware support is not yet a portable product capability

The runtime explicitly marks GPU and NPU discovery as placeholders while also
reporting initialized accelerator devices
([hardware.rs](../../runtime/src/hardware.rs)). The native adapter exposes
placement controls, but the repository's own compatibility and backlog
documents still identify platform and GPU qualification as open.

The practical risk is a misleading capability story:

- a build may compile with a backend but fail to initialize it;
- a device may be detected without verified layer offload;
- memory admission may not know dedicated versus shared memory;
- CPU fallback can be correct but unexpectedly slow;
- behavior differs between Linux, macOS, and Windows builds.

“Supports GPU” should only be claimed per backend, platform, build, and
qualification fixture.

### 8. The runtime policy boundary is not yet actually governing operations

The runtime contains a policy abstraction, but
[security.rs](../../runtime/src/security.rs) describes `FoundationPolicy` as an
explicitly permissive placeholder. The HTTP bridge does enforce bearer tokens
and scopes, which is positive, but the internal policy engine is not yet the
single authority for model acquisition, loading, inference, cancellation,
artifact deletion, or evidence.

This is especially important for an OID client: the model service must not
become an unaudited bypass around approval, policy, and evidence boundaries.
Authentication at the HTTP edge is not the same as authorization integrated
with the governed operation lifecycle.

### 9. Native availability and packaging remain a first-run failure mode

The runtime registers llama.cpp but reports it unavailable unless
`LLAMA_CPP_LIB_DIR` is configured and the native library is present
([runtime README](../../runtime/README.md)). That is appropriate for source
development, but not sufficient for an Ollama-like install experience.

A dependable distribution needs a tested answer for:

- bundled or independently installed native libraries;
- architecture-specific artifacts;
- macOS framework/signing and Windows DLL search behavior;
- model directory initialization;
- first-run download and checksum verification;
- daemon startup and duplicate-daemon handling;
- upgrades and rollback;
- uninstall behavior without deleting user-owned models;
- clean-machine installation with no repository checkout assumptions.

### 10. Qualification evidence is narrower than the product claim

The project has useful scripts for native API, recovery, model profiles,
prompt-cache behavior, installation, and client qualification. However, the
compatibility documents explicitly leave open current-source client
qualification, multiple platform vision flows, tool-stream qualification, and
some release-platform checks.

Unit tests can prove request parsing or scheduler invariants. They do not prove
that a released daemon:

- starts on a clean machine;
- loads a real model through the installed native library;
- streams valid terminal records;
- recovers after client disconnect;
- releases the model and request slot after cancellation;
- survives repeated load/generate/unload cycles;
- behaves consistently through CLI, IPC, Ollama, and OpenAI clients.

## Compatibility matrix

| Capability area | Ollama reference | Talvik/OID current state | Assessment |
|---|---|---|---|
| Text generation | `/api/generate`, streaming and non-streaming | Implemented subset with NDJSON | Good foundation; semantic gaps remain |
| Chat | `/api/chat`, roles, images, tools, structured output | Implemented subset; model/profile gated | Partial and model-dependent |
| Embeddings | `/api/embed`, legacy `/api/embeddings` | Implemented where backend supports embeddings | Partial; dimensions/options/qualification gaps |
| Model list | Tags, names, sizes, digests | Registry-backed `/api/tags` | Partial; identity and metadata are thinner |
| Running models | `/api/ps`, expiry and memory state | Resident model diagnostics | Partial; expiry/VRAM semantics incomplete |
| Model details | `/api/show` with template/parameters/license/info | Registry and selected profile data | Partial; raw metadata absent |
| Pull/download | Manifest-aware resumable model distribution | Raw path/URL acquisition/import | Not equivalent |
| Create | Modelfile/template model creation | Unsupported | Missing |
| Copy/alias | Model copy and naming | Unsupported | Missing |
| Delete | Artifact lifecycle deletion | No public equivalent | Missing |
| Blob protocol | Content-addressed blob checks/upload | Unsupported | Missing |
| Version | `/api/version` | Implemented with Talvik build identity | Compatible shape, different identity |
| Auth | Usually local unauthenticated default | Token and scope checks | Intentional divergence; client impact |
| Hardware | CPU/GPU/platform-specific runtime | CPU-first, accelerator qualification incomplete | Not production-portable yet |
| Service lifecycle | Mature daemon/install/update experience | Exists, but clean-machine and platform gates remain | Productization incomplete |

## Recommended completion definition

The project should not declare “working like Ollama” until all P0 items have
evidence from release artifacts. A practical definition is:

### Minimum local-runner product

- Install on a clean Linux machine without a source checkout.
- Start one daemon deterministically and detect duplicate instances.
- Discover, import, verify, list, inspect, load, run, cancel, unload, and
  remove a local GGUF model.
- Stream valid output and a terminal record for both success and failure.
- Recover from cancellation, disconnect, timeout, malformed input, and model
  load failure without daemon restart.
- Persist model identity and state across daemon restart.
- Report truthful model, backend, placement, memory, and health information.
- Keep all native handles behind the adapter boundary.

### Minimum Ollama compatibility subset

- `/api/version`, `/api/tags`, `/api/ps`, `/api/show`.
- `/api/generate` and `/api/chat`, streaming and non-streaming.
- `/api/embed` and `/api/embeddings` where supported.
- Explicit, tested error behavior for unsupported lifecycle operations.
- A versioned compatibility matrix tied to a specific Ollama client/test
  version.
- At minimum, either implement or explicitly reject with documented errors:
  `keep_alive`, `raw`, `context`, `suffix`, `template`, `think`, and model
  tags.

### Full Ollama-like model management

If the goal is not merely compatibility but an equivalent user experience,
add a separate milestone for:

- manifest/digest-based model identity;
- registry pull/push and progress events;
- blob storage and deduplication;
- Modelfile/create;
- copy/alias;
- safe delete and garbage collection;
- complete `/api/show` metadata;
- keep-alive/expiry semantics;
- broad client qualification.

## Suggested order of work

1. Freeze the product claim as either “partial Ollama compatibility” or “full
   Ollama-like lifecycle.”
2. Close clean-install, native-library, daemon lifecycle, and release-artifact
   qualification gaps.
3. Make model identity, manifest, trust, checksum, ownership, and deletion
   semantics explicit.
4. Complete truthful metadata and residency reporting.
5. Decide which request semantics are implemented versus permanently outside
   scope, then qualify them with real clients.
6. Qualify Linux CPU first, then macOS arm64, Windows CPU, and each accelerator
   backend separately.
7. Only after the local runner is dependable, expand into registry parity,
   broader modalities, or cloud/provider features.

## Bottom line

The core runner is a credible foundation. The project is currently wrong only
if it is being judged as a complete Ollama replacement; under that standard it
is missing model distribution/lifecycle, metadata fidelity, portability, and
release qualification. Under the more accurate standard of a governed Talvik
runtime with an Ollama-shaped edge, the architecture is directionally sound,
but the P0 operational and qualification gaps must be closed before calling it
a working product.

## Implementation follow-up — 2026-11-01

The first P0 installation slice now makes the native embedded chat-template
renderer a required component of installation, includes it in release
archives, and routes the cross-platform release workflow through the shared
platform packagers. Linux DEB/RPM packaging checks the native payload and
declares the C++ runtime dependency. Hardware-free installer and archive tests
cover renderer presence and rejection when missing. This closes a packaging
contract gap in source; the resulting native release artifacts still require
CI build and clean-install smoke evidence before the installation priority can
be marked complete.
