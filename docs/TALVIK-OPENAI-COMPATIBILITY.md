# Talvik OpenAI API Compatibility: Current Surface

Date: 2026-10-29

Reference baseline: [OpenAI Chat Completions API][openai-chat-reference].

[openai-chat-reference]: https://developers.openai.com/api/reference/resources/chat

## Status and scope

Talvik implements a **limited OpenAI-compatible text and local-image API
subset**, not the complete OpenAI API. Native vision inference has been
qualified on Windows only; Linux/macOS and third-party vision-client workflows
remain unqualified. This document describes the current native HTTP behavior
so clients can distinguish supported features, Talvik-specific extensions, and
unsupported or nonconformant behavior.

The primary compatible inference route is `POST /v1/chat/completions`. It
supports text and bounded inline image inputs when the selected local model has
an explicitly paired vision projector. It is not a claim of full wire-level or
semantic compatibility with every OpenAI endpoint, model modality, request
parameter, or SDK feature.

ADR-0031 and WP-0108 define the accepted target and implementation sequence for
broader OpenAI model-serving/inference compatibility. The accepted target does
not include account, organization, billing, fine-tuning, or other
platform-administration APIs.

## Endpoint inventory

| Endpoint | Current status | Behavior and limitations |
|---|---|---|
| `GET /v1/models` | Partial | Returns an OpenAI-shaped `list` with model `id` and `object`, plus Talvik fields `default`, `resident`, and `estimated_memory_mib`. It does not provide the full OpenAI model metadata surface, including `created`. `GET /v1/models/{model}` is not implemented. |
| `POST /v1/chat/completions` | Partial text and local-image subset | Supports one completion per request, non-streaming and SSE streaming, and `response_format` JSON-object or bounded JSON Schema modes. User messages may contain ordered text and bounded base64 data-URL images when a local projector is paired. Remote image URLs are rejected. Request and response differences are listed below. |
| `POST /v1/completions` | Partial | String `prompt` requests now use the legacy `text_completion` response and SSE chunk shapes. Prompt arrays, tools, logprobs, and other unsupported options are rejected; this is not full legacy Completions conformance. |
| `POST /v1/embeddings` | Conditional subset | Accepts a string or an array of strings and returns an OpenAI-shaped embedding list. Availability depends on backend/model support. The current response reports zero prompt/total usage rather than measured token usage, and supports neither multimodal input nor all OpenAI embedding options. |
| `POST /v1/responses` | Partial, text-only | Accepts string `input` and optional string `instructions`; supports standard non-streaming response objects and typed SSE events for streamed text. Input arrays/content parts, tools, continuation, and other Responses features are not implemented. |

Talvik also has local health, readiness, version, cancellation, model-load,
and Reference local runner-style routes. Those are Talvik/runtime interfaces,
not OpenAI API endpoints.

## Chat Completions request support

### Supported core

- `model` is required.
- `messages` supports string content or ordered text/image content parts for
  user messages. Images must use supported base64 data URLs and a vision-capable
  model with an explicitly paired projector; remote image URLs are rejected.
  Assistant messages may also carry tool calls; tool results are translated
  into the model prompt.
- Image inputs are limited to four PNG, JPEG, BMP, or GIF images per request,
  10 MiB of encoded bytes per image, 8192 pixels per dimension, and 16
  megapixels total. Image data remains request-scoped and is not persisted.
- `stream` selects non-streaming or SSE output.
- `max_completion_tokens` and the older `max_tokens` are accepted;
  `max_completion_tokens` takes precedence if both are supplied.
- `temperature`, `top_p`, `seed`, and `stop` (string or string array) are
  mapped to the generation request.
- `tools` is conditionally supported when the loaded GGUF chat template
  advertises tool-call support. Unsupported templates are rejected.

### Talvik-specific request extensions

- `prompt` may be used instead of `messages`.
- `context_size` sets the requested model context.
- `timeout_ms` sets a bounded request timeout.
- `request_id` supplies a caller correlation identifier.
- `top_k` is a Talvik/backend option, not an OpenAI Chat Completions parameter.

### Unsupported or not faithfully implemented

Chat Completions accepts `tool_choice: "auto"`, `"none"`, `"required"`, and
function-specific selection. Forced choices are included in the model prompt
and validated against the generated calls; a model that fails to honor the
choice returns a structured 502 `model_output_error` instead of a successful
text fallback. Stable error codes distinguish incomplete calls, invalid calls,
undeclared tools, and unsatisfied forced choices. Streaming errors use an SSE
`error` event and do not emit `[DONE]`, so a failed tool response is not
presented as a completed stream.
`parallel_tool_calls: false` is prompted and enforced as a single-call limit.
Tool definitions require function-type entries, unique valid names, and
object-valued `parameters`; nested JSON Schema constraints are not validated
by Talvik.
Tool output is model/template dependent. The Windows Qwen evidence below
qualifies only the tested model and tool prompts; cross-platform qualification
remains open.
`response_format` supports `json_object` and a bounded strict `json_schema`
subset (nested objects and arrays, scalar types, enums, and required
properties). Unsupported schema keywords and combining structured output
with tools are rejected. The API does not implement `stream_options`
(including `include_usage`), `logprobs`, `top_logprobs`, `presence_penalty`,
`frequency_penalty`, or `logit_bias`; requests using these recognized options
are rejected with a structured unsupported-feature error. Only `n: 1` is
supported. Unknown fields can still be silently ignored.
Assistant tool-call IDs are checked against tool-result messages, and the IDs
are preserved in the translated prompt. Tool calls are still parsed from
template-formatted model output rather than emitted by a backend-neutral
structured tool interface.

Chat Completions does not support audio or video content, and Responses accepts
only string input and remains text-only. Responses text streaming does not add
array/content-part input, tools, or continuation. Image inference is model- and
projector-dependent and is not yet qualified with third-party OpenAI vision
clients on Linux/macOS. The API does not provide image generation or
reasoning-token features.

Invalid values for validated fields such as `temperature`, `top_p`, token
limits, `stream`, and `stop` are rejected with a structured client error.
Talvik-specific extension fields are not all validated yet; clients should not
assume an undocumented extension was honored.

## Pending compatibility gaps

| Priority | Gap | Remaining work |
|---|---|---|
| P0 | Current-source platform qualification | Re-run the native client workflow on Linux x86_64, macOS arm64, and Windows x86_64 for the current implementation; macOS Intel is also unqualified. The latest Windows OpenAI SDK smoke run did not enable tool probes. |
| P0 | Responses API breadth | Responses currently accepts string input/instructions and returns text. Input arrays/content parts, tools, continuation (`previous_response_id`), response retrieval/deletion, and multimodal input remain unsupported. |
| P0 | Parameter and stream fidelity | `n > 1`, final usage streaming, `stream_options`, log probabilities, penalties, and `logit_bias` remain unsupported. Some unknown fields may be ignored rather than rejected. |
| P0 | Tool-call interoperability | Incremental parsing, validation, and error events have local tests, but tool generation remains model/template/prompt dependent. Re-run both SDK tool-choice/result probes against the current parser; there is no Talvik tool-execution loop or qualified IDE Agent Mode session. |
| P1 | Model and embedding metadata | `/v1/models/{model}` is absent and model metadata is incomplete. Embeddings are text-only; usage is reported as zero, and dimensions and broader embedding options are unsupported. |
| P1 | Media and client workflows | Image input is bounded, local-only, and requires a paired projector; native vision has only been qualified on Windows. Audio/video, image generation, third-party vision-client workflows, and IDE integration remain unqualified. |
| P1 | HTTP transport | The server handles one request per connection and expects `Content-Length`; chunked request bodies and general HTTP keep-alive are outside the current contract. |

## Chat Completions responses

### Non-streaming

The response contains `id`, `object: "chat.completion"`, `model`, `choices`,
and `usage`. Talvik adds `generation_id`, `backend`, `execution`,
`termination_reason`, and `metrics`.

The standard completion envelope includes `created`. `usage.total_tokens` is
derived from Talvik context-token statistics. The single-generation runtime
always returns one choice; only `n: 1` is supported.

Normal finish reasons are `stop`, `length`, and `tool_calls`. Cancellation may
use the Talvik-specific `cancelled` reason. `termination_reason`, `execution`,
and `metrics` are Talvik extensions, not OpenAI schema fields.

### Streaming

Streaming uses Server-Sent Events with `chat.completion.chunk` objects,
`created` metadata, content deltas, a final finish-reason chunk, and
`data: [DONE]`. `stream_options` and the final usage chunk are not implemented.
When tools are supplied, Talvik incrementally parses generated tool markup and
emits a call only after that call's bounded arguments have been completed and
validated. This is not token-by-token argument streaming. If a tool-enabled
request produces ordinary assistant text instead of a parsed tool call, that
text is currently not emitted in the SSE response.

## Errors and HTTP behavior

Recognized request failures, including malformed JSON and a missing required
`model`, return an OpenAI-shaped `error` object with `type`, `message`, `param`,
and `code`. Several error types and status mappings remain Talvik-specific;
unsupported model/backend capabilities may return 409 or 501.

HTTP requests are handled as one request per connection. The native server
expects a `Content-Length` body; chunked request bodies and general HTTP
keep-alive behavior are not part of the compatibility contract.

## Authentication and local deployment

HTTP bearer authentication is enabled by default. Health and readiness routes
are public. HTTP binding is optional and loopback-only unless an explicit
non-loopback override is configured; disabling authentication or allowing a
non-loopback bind is an operator security decision, not an OpenAI compatibility
feature.

## Qualification evidence

[`scripts/qualify-openai-client.py`](../scripts/qualify-openai-client.py)
checks model discovery, non-streaming and SSE chat, non-streaming and SSE
legacy Completions, and non-streaming and SSE text Responses with the OpenAI
Python client. It is a
useful smoke test for that subset, not a full conformance suite. Set
`TALVIK_QUALIFY_TOOLS=true` to exercise automatic, required, and
function-specific tool choices, the single-call constraint, and a correlated
tool-result message round trip. Generated answer quality is not asserted.

[`scripts/qualify-openai-client.mjs`](../scripts/qualify-openai-client.mjs)
uses the official OpenAI JavaScript SDK for model discovery, Chat Completions
streaming/non-streaming, legacy Completions streaming/non-streaming, and
streaming/non-streaming text Responses. The dedicated
[`scripts/qualify-embedding-clients.py`](../scripts/qualify-embedding-clients.py)
and [`scripts/qualify-embedding-clients.mjs`](../scripts/qualify-embedding-clients.mjs)
also test Ollama batch/legacy embeddings and OpenAI batch embeddings using the
official SDKs. Those checks verify nonempty vectors, not OpenAI usage-count
fidelity.

Install the JavaScript SDKs outside the repository and set
`TALVIK_NODE_PREFIX` to the npm prefix:

```powershell
npm.cmd install --prefix $env:TEMP\talvik-clients ollama@0.6.4 openai@7.45.1
$env:TALVIK_NODE_PREFIX = "$env:TEMP\talvik-clients"
node scripts\qualify-openai-client.mjs
```

The manual
`.github/workflows/talvik-api-client-qualification.yml` workflow builds and
exercises the native service with authenticated Python/JavaScript inference
and embedding clients. Run
[40777131756](https://github.com/dcfrancisco/intent-driven-os/actions/runs/40777131756)
passed on Linux x86_64, macOS arm64, and Windows x86_64 at commit
`072b06477529bfcb2ca47a6b80ca0ff1c03a7150`. That run predates the current
tool-call and Windows-path changes; its workflow did not enable the new
tool-call probe or the macOS Intel job. The updated workflow includes both;
re-run it before claiming those changes or that target are qualified.

The tool probe is model-dependent; passing it qualifies only the pinned model
and tested tool round trip, not every model or tool schema. No full OpenAI API
conformance claim is made.

### Windows x86_64 live SDK qualification — 2026-10-29

The latest isolated Windows run served the local Qwen 2.5 0.5B Q4_K_M model
(`qwen2.5-0.5b-instruct-q4_k_m`) on loopback. The official OpenAI Python SDK
3.41.0 and JavaScript SDK 7.45.1 qualification scripts both passed with
`TALVIK_QUALIFY_TOOLS=false`:

- Model discovery, non-streaming and SSE Chat Completions, non-streaming and
  SSE legacy Completions, and non-streaming and SSE text Responses.
- The streamed Responses lifecycle and text-delta events were accepted by both
  official SDKs.

This run did not enable tool probes, test embeddings, or qualify HTTP
authentication. Earlier Windows Qwen probes exercised required and
function-specific tool choices and one automatic-tool SSE response, but those
probes do not requalify the current incremental tool parser. Model-output
failures remain explicit (including HTTP 502 when a required call is absent
or malformed); they are not converted into fabricated successful calls.
Talvik transports tool calls and results but does not execute tools or run an
autonomous agent loop. No interactive Continue/Cline session has been
qualified.

## Compatibility claim

The accurate current claim is:

> Talvik provides a local OpenAI-compatible **text inference subset**:
> Chat Completions with SSE, partial legacy Completions, text-only Responses
> with text SSE,
> plus conditional tool and embedding support. It is not fully compatible
> with the OpenAI API.

See the [capability matrix](./TALVIK-COMPATIBILITY-CAPABILITY-MATRIX.md) and
[WP-0067](./work-packages/WP-0067-assistant-api-compatibility-edge.md) for
related implementation and qualification status.
