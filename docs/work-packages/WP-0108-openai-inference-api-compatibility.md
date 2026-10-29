# WP-0108: OpenAI Inference API Compatibility

## Status

In progress. ADR-0031 accepts the target profile. Talvik implements a partial
text profile covering Chat Completions, legacy Completions, non-streaming
Responses, and conditional embeddings; the endpoint inventory, protocol
implementation, backend capabilities, and qualification remain incomplete.

The implementation distinguishes Chat Completions from legacy Completions,
emits legacy `text_completion` response/chunk shapes for string prompts,
includes `created`, returns structured errors for malformed JSON and missing
required models, rejects several recognized unsupported options, and supports
a non-streaming text-only `/v1/responses` path. Chat Completions now accepts
`tool_choice: "auto"` and `"none"`, preserves tool-call IDs across the
assistant/tool-result prompt translation, and rejects unmatched or
name-mismatched tool results. Required and function-specific choices are
prompted and validated against model output; `parallel_tool_calls: false`
enforces a single-call limit. Chat Completions now incrementally parses
bounded tool-call markup across generation chunks, validates each completed
call before emitting it, and streams OpenAI-shaped tool-call deltas. Local
tests cover fragmented markers and arguments, escaped Unicode, multiple calls,
invalid/incomplete/oversized calls, and SSE delta shape. Responses
streaming/continuation and multimodal inputs remain unimplemented. Forced tool
behavior and the new streaming path await live model and SDK qualification.

The Python and JavaScript qualifiers exercise automatic, required, and
function-specific tool choices, single-call behavior, and the tool-call/result
round trip. The updated code and workflow still require a successful run
before this behavior is considered qualified.

### Qualification update (2026-10-29)

The native matrix passed Linux x86_64, macOS arm64, and Windows x86_64 at
baseline commit `072b06477529bfcb2ca47a6b80ca0ff1c03a7150`
([run 40777131756](https://github.com/dcfrancisco/intent-driven-os/actions/runs/40777131756)).
That run predates the current tool-call changes and did not enable the tool
probe. Re-run the updated workflow after these changes are committed. The
updated workflow also adds the macOS Intel release target, which remains
unqualified until that job passes.

### Windows live probe (2026-10-29)

A native Windows release build succeeded with the installed llama.cpp DLLs.
Raw HTTP requests to local Qwen 2.5 0.5B passed non-streaming required and
function-specific choice with `parallel_tool_calls: false`, each returning
one `get_weather` call. One streaming `auto` request returned a tool-call
chunk and `[DONE]`; that probe predates the incremental parser. The current
local implementation emits validated completed calls during generation, but
has not yet been live-probed. Two streaming required-choice attempts returned
a structured 502 because the model emitted no tool call. The API correctly
avoided a success-shaped text fallback; forced-stream reliability and official
SDK qualification remain open.

## Purpose

Implement and qualify Talvik's OpenAI model-serving/inference API edge while
keeping the canonical runtime API backend-neutral and making unsupported model
capabilities explicit.

## Scope

### Contract and profile

- Pin the OpenAI reference/profile revision and maintain an endpoint/schema
  inventory with supported, partial, unsupported, and qualified status.
- Define backend-neutral request, response, event, capability, and error
  contracts for translation at the HTTP edge.
- Keep account, organization, project, billing, fine-tuning, evaluation
  management, and other platform-administration APIs out of scope.

### Text inference

- Implement `/v1/chat/completions` with profile-conformant request validation,
  standard generation controls, message/tool semantics, response metadata,
  usage, finish reasons, and SSE.
- Implement `/v1/completions` with its own correct request, response, and SSE
  schemas, or explicitly return an unsupported-endpoint error until complete.
- Implement required and function-specific `tool_choice`, `n`,
  structured-output/response-format, and streaming usage semantics where
  supported; return structured errors where the model/backend cannot honor
  them.
- Support all profile-approved text message forms and preserve tool-call IDs
  and tool-result correlation.

### Responses and other inference endpoints

- Add `/v1/responses` request, response, event-stream, cancellation, and
  continuation semantics.
- Complete `/v1/models` discovery metadata and model lookup behavior.
- Complete `/v1/embeddings` inputs, options, response metadata, and measured
  usage for supported embedding models.
- Add image generation/edit/variation and audio transcription/translation/
  speech operations as backend-neutral capabilities with real backend support.
- Add other inference/media endpoints in the pinned profile when their
  backend requirements and acceptance tests are defined.
- Reject unsupported modalities and operations with explicit structured
  errors; do not emit fake results.

### Error, transport, and security behavior

- Normalize malformed JSON, invalid fields, unsupported options, missing
  models, admission failures, timeouts, cancellation, and backend failures to
  profile-conformant status and error bodies.
- Verify HTTP request framing, content length limits, SSE termination,
  disconnect behavior, and query handling against the chosen profile.
- Preserve bearer authentication by default, loopback-only defaults, scoped
  authorization, request ownership, and cancellation policy.

### Qualification

- Add schema/contract fixtures for every endpoint, supported parameter, error
  class, and stream terminal path.
- Qualify using the official OpenAI Python client and raw HTTP/SSE tests.
- Maintain a capability matrix by endpoint, model, backend, and platform.
- Record unsupported features as explicit gaps; do not claim full compatibility
  until every endpoint in the pinned profile passes its acceptance tests.

## Non-goals

- OpenAI account, organization, project, billing, fine-tuning, evaluation
  management, or provider administration.
- Replacing Talvik's backend-neutral internal API with OpenAI protocol types.
- Claiming all models support every API modality.
- Simulating successful image, audio, video, tool, or structured-output
  responses when the selected backend lacks that capability.

## Acceptance criteria

- The compatibility guide identifies the pinned profile and classifies every
  endpoint as supported, partial, unsupported, or qualified.
- `/v1/chat/completions` and `/v1/completions` have distinct correct response
  and streaming shapes.
- Standard parameters are honored or rejected explicitly; invalid recognized
  values never silently become defaults.
- Required response metadata, usage fields, finish reasons, tool IDs, SSE
  events, `[DONE]` behavior, and tool-result correlation match the profile.
- `/v1/responses` and supported embeddings/media endpoints have contract tests
  for non-streaming, streaming, errors, and cancellation.
- Unsupported model/backend capabilities return structured errors and never
  success-shaped placeholders.
- OpenAI Python client tests and raw HTTP/SSE conformance tests pass on each
  supported platform before its status is marked qualified.
- Authentication, authorization, local binding, admission, cancellation,
  recovery, and OID policy regressions are covered.
- Documentation and the capability matrix report the actual qualified
  surface, not the intended future surface.

## Dependencies

ADR-0031; ADR-0013, ADR-0015, ADR-0021, ADR-0025; WP-0067, WP-0091, WP-0092;
backend-neutral modality and capability contracts; native backend support for
each advertised modality.

## Implementation sequence

1. Fix existing text-edge contract defects: validation/errors, distinct
   completions shape, response metadata, standard parameter handling, and SSE.
2. Implement Responses and complete model/embedding contracts.
3. Add image/audio/multimodal capability contracts and backend implementations
   one modality at a time.
4. Run conformance and platform qualification, then update the profile and
   capability matrix.

The sequence is dependency order, not a claim that later modalities are
available now.
