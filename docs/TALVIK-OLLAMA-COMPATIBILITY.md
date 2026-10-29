# Talvik Ollama API Compatibility

Date: 2026-10-28

Reference baseline: [Ollama API documentation](https://docs.ollama.com/api/introduction)
and the linked endpoint references. ADR-0032 and WP-0109 define Talvik's
adopted local model-runner interoperability profile.

## Status

Talvik runs local GGUF models through llama.cpp and exposes an initial
Ollama-shaped **text and local-image inference plus model-introspection
subset**. Inline image parsing and Windows native image inference are tested;
third-party vision-client workflows and Linux/macOS native vision qualification
remain open. This is not a claim of complete Ollama compatibility. Ollama's API
changes over time, and endpoints below are classified by behavior, not merely
by whether a route exists.

| Ollama endpoint | Talvik status | Current behavior / gap |
|---|---|---|
| `GET /api/version` | Partial; Python client qualified | Returns Talvik's package version, not an Ollama release version or complete build identity. |
| `GET /api/tags` | Partial; Python client qualified | Lists registered GGUF models. Local artifact size and modification time are reported when the filesystem provides them; remote or missing files remain unknown. Talvik diagnostics add fields. |
| `GET /api/ps` | Partial; Python client qualified | Lists actual resident models and memory snapshots. Expiration timestamps and VRAM use are not implemented. |
| `POST /api/show` | Partial; Python client qualified | Reports known GGUF family, quantization, and registry-declared capabilities. Includes `model_info: {}` when raw metadata is unavailable. Modelfile, template, license, parameters, and raw GGUF metadata are not available. |
| `POST /api/chat` | Partial; text client qualified, image parser/native path tested | One generation; NDJSON stream by default. User messages may provide bounded base64 images in `images` when a local projector is paired. Some Ollama options, keep-alive controls, prompt templates, thinking, and complete tool semantics are unsupported. |
| `POST /api/generate` | Partial; text client qualified, image parser/native path tested | String prompt, system, and supported generation options; NDJSON stream by default. Bounded base64 `images` are supported when a local projector is paired. Raw mode, context reuse payloads, custom templates, suffix completion, and some standard options are unsupported. |
| `POST /api/embed` | Partial | String or batch text input and embeddings for models/backends that support them. Dimensions, options, truncation policy, per-call keep-alive, and measured prompt token count are not implemented. |
| `POST /api/embeddings` | Partial legacy | Supports the legacy string `prompt` request and single-vector `embedding` response when embeddings are available. |
| `POST /api/create` | Unsupported | No Ollama Modelfile/template build pipeline. |
| `POST /api/copy` | Unsupported | No artifact alias/copy lifecycle contract. |
| `POST /api/pull` | Unsupported as Ollama operation | Talvik can import a raw local path or URL through its own model acquisition API, but does not implement Ollama registry manifests, progress stream, or registry trust semantics. |
| `POST /api/push` | Unsupported | No registry upload or credential contract. |
| `DELETE /api/delete` | Unsupported | There is no public artifact deletion contract with active-use safety and filesystem ownership semantics. |
| `HEAD /api/blobs/{digest}` and `POST /api/blobs/{digest}` | Unsupported | No Ollama blob store protocol. |

Ollama cloud account, balance, web search, and hosted-only features are outside
Talvik's local-runner profile. Modality and tool support are model- and
backend-dependent; unsupported capabilities fail explicitly rather than
returning fabricated results.

## Inference behavior

- The native API uses `/api/chat`, `/api/generate`, `/api/embed`, and legacy
  `/api/embeddings`. OpenAI-shaped routes under `/v1` remain separate.
- `stream` defaults to `true` for `/api/chat` and `/api/generate`. Streaming is
  newline-delimited JSON (`application/x-ndjson`), not SSE. Clients should
  parse one JSON object per line and stop on the record whose `done` is true.
- Chat message content and generate prompts are text; image inputs are passed
  separately as bounded base64 `images` arrays. A compatible local projector
  must be explicitly paired with the selected model. Other modalities are
  rejected.
- Image requests are limited to four PNG, JPEG, BMP, or GIF images, 10 MiB of
  encoded bytes per image, 8192 pixels per dimension, and 16 megapixels total.
  Image data remains request-scoped and is not persisted.
- `options` maps `num_ctx`, `num_predict`, `temperature`, `top_p`, `top_k`,
  `seed`, and `stop` to backend-neutral generation options. Unsupported option
  names are rejected with an Ollama-shaped error.
- `/api/chat` and `/api/generate` currently reject `keep_alive`, structured
  `format`, custom templates, thinking controls, and raw/context modes that
  Talvik cannot honor.
- `tools` are conditional on runtime tool support; this does not qualify every
  tool-call prompt/result round trip or Ollama tool stream behavior.
- `total_duration`, `load_duration`, and available token counts/durations are
  reported from Talvik runtime measurements. Unsupported measurements are
  omitted rather than replaced with zero.
- `/api/embed` returns `model`, `embeddings`, and `total_duration`. A missing
  embedding capability returns an error. Token counts and caller-selected
  embedding dimensions are not yet supported.
- Native API errors use `{ "error": "..." }`. HTTP authentication and scope
  checks remain enabled according to Talvik configuration; Ollama's default
  unauthenticated local-server behavior is not used.

## Model lifecycle and runner gaps

Talvik supports local GGUF discovery/import, registration, load/unload,
residency inspection, resource admission, and bounded recovery internally.
Those are not yet a full substitute for Ollama's model-management API.
Registry pull/push, Modelfile create, model copy/delete, blob distribution,
per-request `keep_alive`, exact `expires_at`, and alias/deduplication behavior
need explicit contracts, artifact integrity, authorization, and recovery tests
before implementation.

## Qualification

The official Ollama Python client 0.6.3 and JavaScript client 0.6.4 passed
against a live authenticated Windows Talvik daemon with the local
`qwen2.5-0.5b-instruct-q4_k_m` GGUF model. Both runs covered `/api/version`,
`/api/tags`, `/api/ps`, `/api/show`, streaming and non-streaming
`/api/generate` and `/api/chat`, and the unknown-model error. The `/api/show`
response includes the schema-required `model_info` object but leaves it empty
because raw GGUF metadata is not exposed.

The official OpenAI Python and JavaScript clients also passed their documented
Windows text-profile smoke runs. The dedicated embedding client checks passed
on Windows for Ollama batch and legacy embeddings and OpenAI batch embeddings
in both SDK languages. OpenAI embedding usage counts remain a known fidelity
gap. Tool-call generation was observed with the test Qwen model, but its
tool-result summary was truncated by the low output limit; tool semantics are
not considered qualified.

Reproduce the Python run with `scripts/qualify-ollama-client.py`, the official
`ollama==0.6.3` package, `TALVIK_API_KEY`, and `TALVIK_MODEL`. The script uses a
512-token context and a four-token output cap to keep qualification within
local resource limits. The official JavaScript client qualifier is
`scripts/qualify-ollama-client.mjs`; install `ollama@0.6.4` outside the
repository and set `TALVIK_NODE_PREFIX` to that npm prefix.

The separate `scripts/qualify-embedding-clients.py` and
`scripts/qualify-embedding-clients.mjs` scripts check Ollama batch and legacy
embeddings plus OpenAI batch embeddings with the official SDKs. Set
`TALVIK_EMBED_MODEL` to a real embedding-capable model; capability and resource
cost are model-dependent.

`.github/workflows/talvik-api-client-qualification.yml` is a manual native
Linux/macOS/Windows qualification workflow. It builds Talvik and llama.cpp,
downloads and SHA-256 verifies the pinned Qwen GGUF, enables bearer
authentication, and runs Python and JavaScript inference and embedding client
qualifiers. Run
[40777131756](https://github.com/dcfrancisco/intent-driven-os/actions/runs/40777131756)
passed on Linux x86_64, macOS arm64, and Windows x86_64 at commit
`072b06477529bfcb2ca47a6b80ca0ff1c03a7150`. The current local changes have not
been rerun on that matrix, and the run did not include the new tool-call probe
or macOS Intel job. The updated workflow includes both. Do not treat the
baseline run as qualification evidence for the current uncommitted changes or
as complete Ollama API compatibility.

The accurate current claim is:

> Talvik provides a partial Ollama-compatible local API for text chat,
> text generation, conditional embeddings, and model metadata. It is not a
> complete Ollama API-compatible runner.
