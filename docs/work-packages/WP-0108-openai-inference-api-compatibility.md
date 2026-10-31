# WP-0108: OpenAI Inference API Compatibility

## Status

In progress. ADR-0031 accepts the target profile. Talvik implements a partial
text profile covering Chat Completions, legacy Completions, text Responses
with SSE, and conditional embeddings; endpoint coverage, parameter fidelity,
backend capabilities, and cross-platform qualification remain incomplete.

The implementation distinguishes Chat Completions from legacy Completions,
emits legacy `text_completion` response/chunk shapes for string prompts,
includes `created`, returns structured errors for malformed JSON and missing
required models, rejects several recognized unsupported options, and supports
non-streaming and typed SSE text responses on `/v1/responses`. Chat Completions now accepts
`tool_choice: "auto"` and `"none"`, preserves tool-call IDs across the
assistant/tool-result prompt translation, and rejects unmatched or
name-mismatched tool results. Required and function-specific choices are
prompted and validated against model output; `parallel_tool_calls: false`
enforces a single-call limit. Chat Completions now incrementally parses
bounded tool-call markup across generation chunks, validates each completed
call before emitting it, and streams OpenAI-shaped tool-call deltas. Local
tests cover fragmented markers and arguments, escaped Unicode, multiple calls,
invalid/incomplete/oversized calls, and SSE delta shape. Model-output failures
now carry specific error codes; streaming failures emit an error event without
a success terminal sentinel. Responses continuation and multimodal inputs
remain unimplemented.

The Python and JavaScript qualifiers exercise automatic, required, and
function-specific tool choices, single-call behavior, and the tool-call/result
round trip. The latest Windows text-profile run passed with tool probes
disabled; re-run both tool probes against the current incremental parser before
considering tool behavior qualified on this source revision.

Chat Completions now supports `response_format` JSON-object mode and strict
`json_schema` mode. The shared schema subset covers nested objects and arrays,
scalar types and enums, and required properties, with explicit complexity
bounds; unsupported schema keywords and combining structured output with
tools are rejected. The llama.cpp adapter compiles these constraints to a
per-request grammar and validates completed output. A Windows native Qwen
qualification passed JSON-object and nested-schema output, cancellation, and a
successful recovery request. Official SDK and cross-platform qualification of
these changes remains outstanding; this does not complete the OpenAI profile.

### Qualification update (2026-10-29)

The baseline native matrix passed Linux x86_64, macOS arm64, and Windows x86_64 at
baseline commit `072b06477529bfcb2ca47a6b80ca0ff1c03a7150`
([run 40777131756](https://github.com/dcfrancisco/intent-driven-os/actions/runs/40777131756)).
That run predates the current tool-call and Responses streaming changes. A
later Windows x86_64 smoke run passed OpenAI Python 3.41.0 and JavaScript
7.45.1 text requests, including streaming Responses, with tool probes disabled.
Re-run the updated workflow on the current source for Linux x86_64, macOS
arm64, and Windows x86_64; macOS Intel also remains unqualified.

### Windows live probe (2026-10-29)

A native Windows release build succeeded with the installed llama.cpp DLLs.
Earlier raw HTTP requests to local Qwen 2.5 0.5B passed non-streaming required
and function-specific choices with `parallel_tool_calls: false`, each
returning one `get_weather` call. One earlier streaming `auto` request returned
a tool-call chunk and `[DONE]`; it predates the incremental parser. Two
earlier streaming required-choice attempts returned a structured 502 because
the model emitted no tool call. The current parser validates each completed
call before emitting it, but current-source live SDK tool probes remain
pending; model-dependent failures must continue to be explicit.

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

### IDE prefill streaming, 2026-10-31

Admitted Chat Completions streams now send HTTP headers and an assistant-role
chunk before waiting for backend tokens, including requests with tools. During
an idle backend wait, SSE comments are flushed every five seconds. These are
transport keep-alives, not generated content, usage, or completion signals.
Request validation and admission still fail before headers; later model-output
failures use SSE error events without a success sentinel. Tool arguments are
still withheld until the existing bounded parser and validation accept them.

The macOS Intel IDE payload contained 59 tools and roughly 22,000 prompt tokens.
A focused optional VS Code agent is supplied in
`docs/examples/vscode/talvik-coding.agent.md` so users can explicitly choose a
smaller tool set on CPU hardware. It preserves normal editor tool approvals.
This is a client configuration option; Talvik does not silently discard schemas
or truncate requests. Full built-in Agent latency remains separately qualified.

### Tool-result continuation correction, 2026-10-31

A real VS Code read-file exchange exposed a continuation that copied Talvik's
synthetic plural `<tool_calls>` history wrapper as visible output. The HTTP
edge now carries assistant tool-call history as a structured private record.
The native adapter renders each past call using the loaded profile's JSON or
function/parameter protocol, preserving accompanying assistant text and call
IDs. Function/parameter history escapes XML text and rejects names that its
protocol cannot represent. Tool instructions now explicitly prefer using
available results over repeating completed calls. This does not execute calls
or relax validation of calls emitted by the model. Live continuation behavior
must be rechecked; this source change alone is not proof of a fix.

For templates declaring both `<tool_response>` markers and a supported tool
format, tool results now use that response wrapper under a user role. The
role/content-only native C renderer otherwise misses the full Jinja tool
branch and emits a literal tool role. Other templates retain their tool role.
