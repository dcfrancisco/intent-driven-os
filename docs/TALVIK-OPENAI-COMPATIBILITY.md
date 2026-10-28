# Talvik OpenAI API Compatibility: Current Surface

Date: 2026-10-28

Reference baseline: [OpenAI Chat Completions API][openai-chat-reference].

[openai-chat-reference]: https://developers.openai.com/api/reference/resources/chat

## Status and scope

Talvik implements a **limited OpenAI-compatible text API subset**, not the
complete OpenAI API. This document describes the current native HTTP behavior
so clients can distinguish supported features, Talvik-specific extensions, and
unsupported or nonconformant behavior.

The primary compatible inference route is `POST /v1/chat/completions`. The
route is intended for local assistant clients using text models. It is not a
claim of full wire-level or semantic compatibility with every OpenAI endpoint,
model modality, request parameter, or SDK feature.

ADR-0031 and WP-0108 define the accepted target and implementation sequence for
broader OpenAI model-serving/inference compatibility. The accepted target does
not include account, organization, billing, fine-tuning, or other
platform-administration APIs.

## Endpoint inventory

| Endpoint | Current status | Behavior and limitations |
|---|---|---|
| `GET /v1/models` | Partial | Returns an OpenAI-shaped `list` with model `id` and `object`, plus Talvik fields `default`, `resident`, and `estimated_memory_mib`. It does not provide the full OpenAI model metadata surface, including `created`. `GET /v1/models/{model}` is not implemented. |
| `POST /v1/chat/completions` | Supported text subset | Supports one text completion per request, non-streaming and SSE streaming. Request and response differences are listed below. |
| `POST /v1/completions` | Partial | String `prompt` requests now use the legacy `text_completion` response and SSE chunk shapes. Prompt arrays, tools, logprobs, and other unsupported options are rejected; this is not full legacy Completions conformance. |
| `POST /v1/embeddings` | Conditional subset | Accepts a string or an array of strings and returns an OpenAI-shaped embedding list. Availability depends on backend/model support. The current response reports zero prompt/total usage rather than measured token usage, and supports neither multimodal input nor all OpenAI embedding options. |
| `POST /v1/responses` | Partial, text-only | Accepts string `input` and optional string `instructions`, returns one standard-shaped text message. Streaming, input arrays/content parts, tools, continuation, and other Responses features are not implemented. |

Talvik also has local health, readiness, version, cancellation, model-load,
and Reference local runner-style routes. Those are Talvik/runtime interfaces,
not OpenAI API endpoints.

## Chat Completions request support

### Supported core

- `model` is required.
- `messages` supports a list of role/content messages with **string content**.
  Assistant messages may also carry tool calls; tool results are translated
  into the model prompt.
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
choice returns a structured 502 error instead of a successful text fallback.
`parallel_tool_calls: false` is prompted and enforced as a single-call limit.
Tool definitions require function-type entries, unique valid names, and
object-valued `parameters`; nested JSON Schema constraints are not validated
by Talvik.
Tool output is model/template dependent and these behaviors still need live
Windows and cross-platform qualification.
The API currently does not implement `response_format`,
`stream_options` (including `include_usage`), `logprobs`, `top_logprobs`,
`presence_penalty`, `frequency_penalty`, or `logit_bias`; requests using these
recognized options are rejected with a structured unsupported-feature error.
Only `n: 1` is supported. Unknown fields can still be silently ignored.
Assistant tool-call IDs are checked against tool-result messages, and the IDs
are preserved in the translated prompt. Tool calls are still parsed from
template-formatted model output rather than emitted by a backend-neutral
structured tool interface.

Message content is limited to strings; array-valued content parts such as
images and audio are not supported. The API does not provide multimodal
generation, audio, image, video, or reasoning-token features. Responses accepts
only string input and does not support streaming or tools.

Invalid values for validated fields such as `temperature`, `top_p`, token
limits, `stream`, and `stop` are rejected with a structured client error.
Talvik-specific extension fields are not all validated yet; clients should not
assume an undocumented extension was honored.

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
When tools are supplied, generation text is buffered and normalized tool calls
are returned after generation; this is not incremental OpenAI tool-call
streaming. If a tool-enabled streaming request produces ordinary assistant
text instead of a parsed tool call, that text is currently not emitted in the
SSE response.

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
legacy Completions, and text Responses with the OpenAI Python client. It is a
useful smoke test for that subset, not a full conformance suite. Set
`TALVIK_QUALIFY_TOOLS=true` to exercise automatic, required, and
function-specific tool choices, the single-call constraint, and a correlated
tool-result message round trip. Generated answer quality is not asserted.

[`scripts/qualify-openai-client.mjs`](../scripts/qualify-openai-client.mjs)
uses the official OpenAI JavaScript SDK for model discovery, Chat Completions
streaming/non-streaming, legacy Completions streaming/non-streaming, and
non-streaming Responses. The dedicated
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

On Windows x86_64, raw HTTP requests against a local Qwen 2.5 0.5B model
returned one `get_weather` call for non-streaming `required` and
function-specific choices with `parallel_tool_calls: false`. One streaming
`auto` request returned a tool-call chunk followed by `[DONE]`. Two streaming
`required` attempts returned a structured 502 because that model did not emit
a tool call. Tool calls are buffered rather than emitted incrementally; forced
streaming remains model-dependent, and the official SDK probes still require
qualification.

## Compatibility claim

The accurate current claim is:

> Talvik provides a local OpenAI-compatible **text inference subset**:
> Chat Completions with SSE, partial legacy Completions, text-only Responses,
> plus conditional tool and embedding support. It is not fully compatible
> with the OpenAI API.

See the [capability matrix](./TALVIK-COMPATIBILITY-CAPABILITY-MATRIX.md) and
[WP-0067](./work-packages/WP-0067-assistant-api-compatibility-edge.md) for
related implementation and qualification status.
