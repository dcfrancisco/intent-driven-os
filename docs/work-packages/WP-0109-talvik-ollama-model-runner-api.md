# WP-0109: Talvik Ollama-Compatible Model Runner API

## Status

In progress. Phase 1 route semantics are implemented. The official Ollama
Python 0.6.3 and JavaScript 0.6.4 clients, and the OpenAI Python 3.40.0 and
JavaScript 7.45.1 text-profile clients, passed their Windows smoke runs against
a local Qwen GGUF. Ollama and OpenAI batch embedding plus legacy Ollama
embedding checks also passed on Windows through both SDK languages. The native
matrix passed Linux x86_64, macOS arm64, and Windows x86_64 at baseline commit
`072b06477529bfcb2ca47a6b80ca0ff1c03a7150`
([run 40777131756](https://github.com/dcfrancisco/intent-driven-os/actions/runs/40777131756)).
The current local changes and tool-call probe have not been requalified.
Phase 2 remains gated on a passing run for those changes and the newly added
macOS Intel matrix job. Talvik is not yet a complete Ollama-compatible runner.

### Qualification update (2026-10-29)

The baseline matrix run completed successfully, but it does not qualify the
current local changes:

1. Commit and push the current tool-call, per-model capability, and Windows
   path fixes with the updated workflow.
2. Re-run `.github/workflows/talvik-api-client-qualification.yml`; inspect the
   job results and uploaded logs, including the tool probes.
3. Qualify the newly added macOS Intel job in addition to the existing
   Linux/macOS arm64/Windows matrix.
4. Only after those gates pass, proceed to verified pull, then safe copy/delete
   with integrity, ownership, active-use, pinning, and recovery tests.

The workflow installs client SDKs, creates an ephemeral test token, and
downloads/verifies its pinned Qwen GGUF. The Windows SDK installation and
qualification daemon used in this session were temporary and are not required
for the handoff.

## Gap inventory

| Area | Talvik today | Ollama-compatible target | Priority |
|---|---|---|---|
| Inference | Native `/api/chat` and `/api/generate` text subset implemented; Ollama Python client passed on Windows | Full supported option, stream, cancellation, error, and cross-platform semantics | P0 |
| Embeddings | Conditional `/api/embed`, legacy `/api/embeddings`, and `/v1/embeddings` implemented | Real-model qualification; dimensions, truncation/options, and measured OpenAI usage remain gaps | P0 |
| Discovery/inspection | `/api/tags`, `/api/ps`, `/api/show`, `/api/version`; Python client passed on Windows | Exact metadata fields and cross-platform client qualification | P0 |
| Residency | Load/unload and global idle-retention policy; no per-request keep-alive | Safe keep-alive/expiry contract and truthful `expires_at` | P1 |
| Artifact acquisition | Internal raw local/URL GGUF import with checksum | Ollama registry pull/progress and manifest semantics | P1 |
| Model definition/replication | No Modelfile, copy, push, or blob API | Create/copy/push/delete/blob flows, persistent manifest and artifact integrity | P1 |
| Model capability | CPU-first GGUF text generation; conditional embeddings/tools | Capability-aware options and modality-specific endpoints; no false capability claims | P1 |
| Transport/errors | Native HTTP; request-per-connection; partial error mapping | NDJSON framing, Ollama error body, streaming failures, request limits and cancellation behavior | P0 |
| Qualification | Baseline native run passed Ollama Python/JavaScript inference and OpenAI/Ollama embedding checks on Linux x86_64, macOS arm64, and Windows x86_64 | Current-change and macOS Intel evidence, plus raw endpoint/stream fixtures | P0 |

## Scope and phases

### Phase 1: text semantics and client qualification

- Implement `/api/chat` and `/api/generate` for text requests using the
  backend-neutral `RuntimeService` and GGUF generation.
- Emit Ollama-compatible NDJSON when `stream` is true; default streaming
  semantics match the Ollama API. Non-streaming responses include assistant
  content, `done`, and measured statistics when available.
- Map supported `options` to `GenerationOptions`; reject recognized
  unsupported controls rather than silently ignoring them.
- Implement `/api/embed` batch and string inputs through model-supported
  embeddings, and legacy `/api/embeddings` request/response aliases.
- Implement `/api/show` from registry metadata, omitting unavailable template,
  license, parameters, and raw GGUF model-info fields.
- Expose `/api/version` using Talvik build identity while preserving
  `/version` and `/v1/version`.
- Normalize native-route errors to Ollama's `{ "error": "..." }` shape without
  changing OpenAI `/v1` errors.
- Keep authenticated scopes and local-only defaults.
- The official Ollama Python client 0.6.3 passes against a live Windows Talvik
  daemon for version, tags, ps, show, streaming and non-streaming chat/generate,
  and missing-model errors. The qualification script is
  `scripts/qualify-ollama-client.py`.
- The official Ollama JavaScript client 0.6.4 passed the same Windows
  discovery, inspection, stream/non-stream, and missing-model checks through
  `scripts/qualify-ollama-client.mjs`.
- The official OpenAI Python 3.40.0 and JavaScript 7.45.1 clients passed
  Windows smoke checks for model discovery, Chat Completions, legacy
  Completions streaming/non-streaming, and non-streaming Responses. Tool
  generation remains model-dependent and is an opt-in probe, not a fully
  qualified semantic contract.
- `scripts/qualify-embedding-clients.py` and
  `scripts/qualify-embedding-clients.mjs` qualify Ollama batch/legacy
  embeddings and OpenAI batch embeddings with their official SDKs. Both passed
  on Windows with the local Qwen GGUF; OpenAI usage-count fidelity remains an
  implementation gap.
- `.github/workflows/talvik-api-client-qualification.yml` builds the native
  daemon and runs both Ollama and OpenAI clients on Linux, macOS, and Windows
  with a pinned Qwen GGUF artifact. Run the workflow and retain its per-platform
  logs before marking those platforms qualified.
- The matrix uses the pinned Qwen model for the embedding client checks;
  additional model/backend capability and resource cost still vary.
- Close any request, response, stream, and error mismatch found by those
  clients before beginning model-management API work.

### Phase 2: verified pull and safe local model operations

- Implement verified model pull with digest checks, progress, interruption
  recovery, storage bounds, and explicit source trust.
- Add safe delete/copy operations with active-request and pinning protections,
  durable registry updates, and explicit filesystem artifact ownership.
- Do not begin these lifecycle operations until the JavaScript client run,
  embedding checks, and native client workflow have successful evidence on all
  supported platforms.
- Define per-request `keep_alive` only if runtime scheduling safely supports
  model-specific expiry. Report `expires_at`, size, and memory only when
  actually measured or configured.

### Separate future slices: model definitions and publishing

- Modelfile create, registry push, and blob distribution require separate
  runtime/artifact contracts and work packages; they are not prerequisites for
  completing Phase 1 or 2.
- Reuse artifact integrity and model-registry contracts; do not make a remote
  URL an implicit trusted model.

### Separate future slice: capability expansion

- Add image/audio/tool/structured-output/thinking support only through
  backend-neutral capability contracts and qualified native backends.
- Return explicit unsupported-capability errors for models that lack support.

## Non-goals

- Ollama cloud account, billing, hosted model access, or web search.
- Replacing Talvik's canonical API or scheduler with Ollama internals.
- Returning fake success for unsupported model operations, metadata, or
  modalities.
- Claiming all local models support the same options or capabilities.

## Acceptance criteria

- Every official local Ollama API endpoint is listed in the compatibility
  guide and classified as supported, partial, unsupported, or out of scope.
- Chat, generate, and embed requests match their schemas for valid inputs,
  invalid inputs, streaming, non-streaming, cancellation, and backend errors.
- NDJSON stream emits complete valid JSON lines and exactly one terminal
  record; usage/timing is measured or explicitly omitted.
- Supported option mappings have unit tests; unsupported recognized options
  fail explicitly.
- Model inspection and list routes report only truthful registry/runtime data.
- Lifecycle/distribution routes are not marked supported until their artifact
  integrity, authorization, active-use, and recovery tests pass.
- Ollama Python and JavaScript clients are qualified against Talvik on
  supported platforms; a passing unit test alone is not client qualification.
- The docs and matrix reflect actual evidence; no full-compatibility claim is
  made while any required profile operation is partial or unsupported.

## Dependencies

ADR-0032; ADR-0003, ADR-0021, ADR-0025, ADR-0031; WP-0058, WP-0061, WP-0062,
WP-0091, WP-0093, WP-0096, WP-0097, WP-0108; runtime-owned option,
capability, artifact, and lifecycle contracts.

## Initial implementation slice

Implemented in the first slice: text `/api/chat` and `/api/generate` with
NDJSON streaming, supported sampler options, explicit unsupported-feature
errors, `/api/embed` and legacy `/api/embeddings`, metadata-backed `/api/show`,
and `/api/version`. `/api/tags` now reports local file size and modification
time when available. `/api/show` includes the SDK-required `model_info` field
as an empty object when raw GGUF metadata is unavailable. Focused HTTP tests
pass. The Ollama Python and JavaScript text clients and Ollama/OpenAI Python
and JavaScript embedding checks passed on Windows; OpenAI Python and JavaScript
text smoke runs also passed. The native Linux/macOS/Windows workflow is
available and its baseline run passed on Linux x86_64, macOS arm64, and Windows
x86_64. The current local changes still need qualification, and the matrix
now includes macOS Intel, which also needs a passing run. The current agreed
order is to finish those qualification gates before beginning verified pull
and safe delete/copy.

Create/Modelfile, push, blobs, full verbose model metadata, per-call keep-alive,
and multimodal features remain separate gaps until their runtime contracts
are implemented and tested.
