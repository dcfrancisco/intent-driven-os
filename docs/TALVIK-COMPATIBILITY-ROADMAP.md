# Talvik Compatibility and Differentiation Roadmap

Status update, 2026-11-01: [current report](reports/TALVIK-STATUS-2026-11-01.md). Native embedded Jinja execution is committed in `5d5925b`; full VS Code build execution, current full-request latency parity and cross-platform RC gates remain open. The implementation slice below records the earlier roadmap baseline, not a replacement for current per-build evidence.

Date: 2026-10-29

## Product target

Talvik aims for high-fidelity interoperability with the declared local model
inference surfaces of the OpenAI API and Ollama. Compatibility means that
clients can rely on the documented endpoint semantics, schemas, streams,
errors, cancellation, and model-dependent capabilities. It does not mean
reimplementing either product, its user interface, hosted services, or
internal architecture.

The target excludes OpenAI account, billing, fine-tuning, and other
platform-administration APIs, along with Ollama cloud account and hosted
features. Those boundaries are detailed in
[WP-0108](./work-packages/WP-0108-openai-inference-api-compatibility.md) and
[WP-0109](./work-packages/WP-0109-talvik-ollama-model-runner-api.md).

Compatibility claims must name the API profile, endpoint, model/backend, SDK,
and platform actually qualified. An endpoint existing is not proof of
compatibility, and a model capability must never be reported merely because a
different resident model supports it.

## Current implementation slice

The OpenAI Chat Completions edge accepts automatic, disabled, required, and
function-specific tool choice. Forced choices are prompted and validated
against model output; `parallel_tool_calls: false` is enforced. Tool-call IDs
are distinct and incoming results are correlated to preceding assistant calls.
The Python and JavaScript qualifiers exercise the choice modes and result
round trip. Tool capability is queried for the selected model, and Windows
local-path and standard `file://` model acquisition is covered by runtime
tests.

Focused Windows HTTP tests pass. A local Qwen 2.5 0.5B passed raw
non-streaming required and function-specific choice probes. One streaming
`auto` request returned a tool-call chunk and `[DONE]`; two streaming
`required` attempts returned a 502 when the model did not emit a tool call.
Official SDK probes, dependable forced-choice behavior, incremental
tool-call streaming, broader Responses semantics, and a real VS Code Agent
session remain open.

## Build order

### P0 — Complete client and platform qualification

- Run the Python and JavaScript OpenAI and Ollama clients, including embedding
  and tool-call probes, on each supported native platform.
- Keep evidence per OS, architecture, SDK version, model, and API route.
- Run the newly added native macOS Intel job as well as Linux x86_64, macOS
  arm64, and Windows x86_64; the previous matrix run covered only the latter
  three targets.
- Qualify an actual VS Code Custom Endpoint text-chat session and, separately,
  an Agent-mode tool-call round trip. SDK success alone does not establish IDE
  compatibility.
- Fix platform-specific failures before starting lifecycle API work.

### P0 — Finish tool-call contract and model capability correctness

- Qualify forced/function-specific choices and single-call behavior against
  real models and clients; validate tool-definition shape and incomplete
  histories. Nested JSON Schema constraints remain unvalidated.
- Correlate tool-call IDs and results across parallel calls and verify
  streaming terminal behavior.
- Qualify the selected-model tool capability check, which now replaces the
  previous aggregate of resident models; see CAP-038 in the
  [capability matrix](./TALVIK-COMPATIBILITY-CAPABILITY-MATRIX.md).
- Test tools against multiple tool-capable and non-tool-capable templates.
  Model-generated function quality is not fully deterministic, so qualification
  must separate protocol behavior from model quality.

### P1 — Complete the declared OpenAI inference profile

- Finish Chat Completions parameter validation, standard response metadata,
  usage accounting, errors, tool-call streaming, and recognized-field handling.
- Complete legacy Completions or narrow the published compatibility claim to
  the exact supported subset.
- Implement Responses streaming, continuation, and tools, or return clear
  unsupported errors and keep those features outside the completed profile.
- Report truthful model metadata and measured embedding usage; define
  dimensions and input constraints.
- Add multimodal inference only when a backend-neutral contract and a real
  capable backend exist. Do not claim image/audio/video support from text-only
  paths.

### P1 — Complete the local Ollama inference profile

- Close `/api/chat`, `/api/generate`, and embedding option, stream, error,
  cancellation, and metadata gaps.
- Define truthful `keep_alive`/expiry behavior and expose only measured or
  configured residency data.
- Verify interoperability against official Python and JavaScript clients and
  raw protocol fixtures on every supported platform.

### P1 — Add safe Ollama-shaped artifact lifecycle operations

After the client/platform qualification gate passes:

1. Implement verified pull with trusted source policy, digest checks,
   bounded storage, progress, interruption recovery, and durable registry
   updates.
2. Implement copy and delete with artifact ownership, alias semantics,
   active-request and pinning protection, authorization, and recovery tests.
3. Keep Modelfile creation, registry push, and blob distribution in separate
   slices because they need their own artifact and credential contracts.

### P2 — Expand backends and modalities deliberately

- Add another inference engine only through a backend-neutral adapter and
  capability contract.
- Add image, audio, video, structured-output, and other model-specific features
  only when their semantics are implemented and qualified end to end.
- Keep cloud-provider credential routing separate from local inference and
  define its policy, isolation, health, and cost contracts before adding it.

## Talvik differentiators to build on

Compatibility is the dependable entry point; Talvik's value should come from
runtime behavior around the model:

- Explainable model selection based on policy, model capabilities, health, and
  available memory.
- Safe admission, bounded queues, cancellation, and recovery under resource
  pressure.
- Verified artifact acquisition, provenance, explicit ownership, and safe
  lifecycle operations.
- Per-client authorization, request ownership, and useful diagnostics without
  logging prompts, responses, or credentials.
- A stable native API for capabilities that do not fit an OpenAI or Ollama
  compatibility edge.

Some foundations already exist, including local bearer authentication,
resource admission, request cancellation, runtime metrics, model residency,
and recovery logic. Each still needs the contract and platform evidence named
in its work package before it is advertised as production-ready.

## Acceptance rule

Talvik may claim a profile complete only when every in-scope endpoint and
behavior is either implemented and qualified or explicitly excluded by the
published profile. Unsupported recognized features must fail clearly; they
must not be silently ignored or return success-shaped placeholders.

Detailed endpoint status and evidence:

- [OpenAI compatibility](./TALVIK-OPENAI-COMPATIBILITY.md)
- [Ollama compatibility](./TALVIK-OLLAMA-COMPATIBILITY.md)
- [Capability matrix](./TALVIK-COMPATIBILITY-CAPABILITY-MATRIX.md)
