# Talvik local API contract

Scope: local CPU API stabilization candidate, 2026-10-31. This is an explicit
compatibility subset, not a claim of complete OpenAI or Ollama parity.

## Connection and authentication

Use the configured loopback address (the qualified local installation uses
`http://127.0.0.1:11435/v1`). Authenticate with `Authorization: Bearer <token>`;
keep tokens outside source control. Health/readiness routes are unauthenticated.
Model discovery requires `models:read`; inference requires `inference`.
Cancellation requires the caller's cancellation scope or cancellation-any scope.
HTTP requests use Content-Length bodies and Connection: close responses. Do not
half-close the request socket while waiting for a response: EOF is treated as
client cancellation. Chunked request bodies and HTTP pipelining are not qualified.

## Route inventory

| Route | Contract / limit | Evidence |
|---|---|---|
| `GET /healthz`, `/readyz` | Process health versus current model-residency readiness | Local service checks |
| `GET /v1/version` | Build identity | Local installed build checks |
| `GET /v1/models` | Registered model list, default/residency/memory hints | Official Python SDK discovery |
| `POST /v1/chat/completions` | Text and inline image messages; stream or JSON; model-dependent tools | Native API suite and Python SDK |
| `POST /v1/completions` | Legacy text generation, stream or JSON | Python SDK |
| `POST /v1/responses` | Stateless text subset, stream or JSON; tools not supported | Python SDK |
| `POST /v1/embeddings` | Backend/model-dependent embeddings | Implemented; not qualified with current Qwen generation models |
| `POST /v1/models/{id}/load` | Runtime-owned model admission | Implemented; separate from OpenAI compatibility |
| `POST /v1/models/{id}/context/reset` | Reset model prompt cache | Implemented; separate from OpenAI compatibility |
| `POST /v1/generations/{request_id}/cancel` | Cancel an authorized active/queued request | Native recovery suite |
| `GET /api/version`, `/api/tags`, `/api/ps`; `POST /api/show` | Ollama-style discovery and residency subset | Implemented; `/api/ps` used by recovery suite |
| `POST /api/chat`, `/api/generate` | Ollama-style generation and NDJSON | Implemented; native `/api/generate` disconnect recovery checked |
| `POST /api/embed`, `/api/embeddings` | Backend/model-dependent embeddings subset | Implemented; current model profile not qualified |

Implemented routes without listed live client evidence are not promoted to the
qualified client contract. Responses storage/retrieval, hosted tools, model
creation/download parity, audio and other unlisted OpenAI surfaces are outside
this candidate. Use Chat Completions for coding-assistant tool calls.

## Generation and streaming

Specify an installed `model` ID. Chat Completions accepts `messages`, `stream`,
`max_tokens` or `max_completion_tokens`, sampling controls and `stop`. Context
and image/tool capabilities must fit the selected model; small text models do
not acquire tool or vision support through client configuration.

The Talvik `context_size` extension is bounded to 32,768 tokens. Daemon defaults
are controlled by `TALVIK_HTTP_CONTEXT_SIZE`, `TALVIK_HTTP_MAX_TOKENS`, and
`TALVIK_ENABLE_THINKING`; per-request values override applicable defaults.
An optional `timeout_ms` (1–300,000) bounds the generation/queue wait after model
loading and admission setup. It is not an end-to-end model-download/load SLA.

SSE emits `data:` JSON frames, a final choice finish reason, optional requested
usage, then `data: [DONE]` on success. With
`stream_options: {"include_usage": true}`, a usage frame has `choices: []`;
clients must accept it. Model output or runtime failures after headers are sent
are error events rather than successful completions. Responses uses its own
named lifecycle events. Do not parse all three stream formats identically.

`finish_reason: length` covers output/context exhaustion; Talvik's termination
metadata distinguishes the cause. Tool calls use correlated IDs and JSON-string
arguments; the client executes tools and supplies their results. Talvik never
executes filesystem tools merely because a model requests one.

## Failure and recovery

| Status | Meaning |
|---|---|
| 400 | Invalid input, including prompt context overflow |
| 401 / 403 | Missing credentials / insufficient scope |
| 404 | Unknown model or resource |
| 409 | Conflicting lifecycle or request ID already in use |
| 413 | Request body exceeds 56 MiB |
| 429 | Retryable runtime admission / full generation queue |
| 500 | Backend/internal failure |
| 501 | Explicitly unsupported feature |
| 503 | Unavailable backend or non-retryable resource admission |
| 504 | Generation deadline expired before response headers |

OpenAI-style errors contain an `error` object with a human-readable `message`.
Streaming failures after HTTP 200 are conveyed inside the stream. One generation
runs at a time; the configured bounded FIFO queue handles additional work.
Never assume a new model can load while another model is busy.

Transport errors cancel their generation and release request ownership. Waiting
handlers check connection closure every 100 ms, including before the first
model token, and use a five-second socket write timeout. Native cancellation
still needs the current backend work unit to finish; cleanup waits up to 30 s.
Request IDs are reserved until their handler exits, so a duplicate cannot steal
another caller's cancellation ownership. Closing or cancelling a request should
allow a subsequent request without a daemon restart.

## Executable qualification

- `scripts/test-talvik-native-api.py`: chat, coding, vision, SSE tool/result loop.
- `scripts/test-talvik-api-recovery.py`: authentication, validation, limits,
  deadline/disconnect recovery, bounded queue overflow, duplicate request IDs,
  queued cancellation and subsequent completion. Requires an idle daemon.
- `scripts/qualify-openai-client.py`: official Python SDK discovery and streamed/
  non-streamed Chat Completions, legacy Completions and text Responses.

Use private token files/environment variables; these scripts do not print bearer
tokens. The recovery harness cancels only its own generated request IDs, but its
load test must run on an idle/disposable instance. Qualification is recorded
per build; passing these suites does not establish VS Code Agent/Plan success,
GPU correctness, clean-machine installation, or other-platform release status.

`scripts/test-talvik-resource-profile.py` uses an isolated native daemon with a
controlled memory budget. It checks manual load/unload readiness transitions,
rejects a large context, then completes twelve short serial requests. It requires
the canonical installed daemon idle with its DeepSeek fixture resident and does
not exhaust OS memory. `TALVIK_TEST_OUTPUT_DIR` selects its artifact directory.
