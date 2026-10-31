# Runtime Crate

Phase 1 runtime crate boundaries:

- `core` — orchestration and public runtime handle
- `config` — deterministic configuration loading boundary
- `logging` — logging initialization boundary
- `lifecycle` — explicit startup/shutdown state machine
- `hardware` — CPU/GPU/NPU discovery interface
- `backends` — replaceable inference adapter interface
- `models` — model registry interface
- `security` — principal and policy interfaces
- `api` — transport-neutral runtime API
- `adapters/llama-cpp` — public package boundary for the native adapter
- `adapters/llama-cpp/sys` — opt-in direct FFI/link boundary for libllama

The console consumes `RuntimeService` from `MarinaRuntime`. `Runtime::start` is
the single production composition root; it validates configuration and creates
the service with the backend manager, model registry, hardware service, and
event bus. Test-only doubles remain available where individual contracts need
deterministic isolation.

## Phase 5 services

`BackendManager` owns registration, enablement, selection, and health for
backend adapters. `ModelRegistry` stores model metadata only and can persist it
to a registry file. `HardwareService` reports portable system facts and leaves
GPU/NPU discovery as explicit placeholders. All lifecycle changes flow through
the shared `EventBus`.

The runtime registers `LlamaCppAdapter` at startup. If `LLAMA_CPP_LIB_DIR` is
not configured, it remains registered and reports `Unavailable`; there is no
silent mock fallback. Configured builds link directly to `libllama` and never
launch `llama-cli`.

The runtime recursively discovers `.gguf` files from configured local model
roots, including common Hugging Face/Python and Reference local runner cache directories.
`ModelAcquirer` imports local paths, `file://` URLs, or explicit HTTP(S) URLs
into the managed store with atomic writes and optional SHA-256 verification.
The registry persists when Marina is started through `config::load()`.

The current runner supports one-model-at-a-time loading/unloading,
backend-reported model memory, native tokenizer routing, text generation,
streaming, and cancellation. Chat, embeddings, and tool execution remain
separate capabilities.

Generation is represented by `GenerationRequest`, `GenerationOptions`,
`GenerationStream`, `GenerationResult`, and `GenerationStatistics`. A runtime
client receives the stream immediately; a worker publishes token events while
the client consumes output. Cancellation is an atomic request signal and does
not stop the process or unload the model.

## IDE access today

The current usable IDE path is a loopback OpenAI-compatible bridge:

```sh
talvik auth token create ide
python3 scripts/marina-openai-proxy.py
```

Configure the IDE with base URL `http://127.0.0.1:11435/v1`, the printed value
as its Bearer/API key, and a model returned by `GET /v1/models`. The bridge
supports both non-streaming and server-sent-event (`stream: true`) text
responses. The token file is `$HOME/.marina/tokens` by default and is created
with user-only permissions.
Set `MARINA_TOKEN_FILE` to use another secret-store integration point.

Persistent runner settings belong in `$HOME/.marina/config.yaml` on Unix or
`%USERPROFILE%\.marina\config.yaml` on Windows. Environment variables override
file values for tests and deployment automation. Talvik loads the configured
default model on daemon startup; by default it is Gemma 4 E4B-it Q4_0 with its
vision projector. If missing, Talvik pulls both pinned artifacts (about
4.8 GiB combined), verifies their SHA-256 checksums, pairs them, and loads the
model. Set `default_model: none` to opt out, or configure a smaller default
model. Failures are logged as warnings and do not prevent the daemon from starting.
Bootstrap runs after the local IPC and HTTP listeners are available. `/healthz`
remains useful during provisioning or failure, while `/readyz` succeeds only
after a model has actually loaded. Set `TALVIK_AUTO_BOOTSTRAP=false`,
`TALVIK_AUTO_DOWNLOAD=false`, or `TALVIK_AUTO_LOAD=false` to disable the
corresponding startup action.

## Native HTTP API

The native HTTP server is disabled unless explicitly configured. Enable a
loopback listener with:

```sh
export MARINA_HTTP_ADDR=127.0.0.1:11434
marina
```

HTTP API routes require a Marina bearer token by default. The current native
surface includes `/healthz`, `/v1/models`, `/v1/models/{id}/load`,
`/v1/chat/completions`, `/v1/completions`, and request cancellation at
`/v1/generations/{request_id}/cancel`. The existing Unix socket remains the
default local transport and uses the same `RuntimeService` application layer.
`GET /v1/models` reports whether a model is resident, its conservative
`estimated_memory_mib`, and the configured default model. `oid-marina-chat`
honors `OID_MARINA_MODEL`, otherwise selects the configured default, then a
resident model, then the model with the smallest reported estimate. These
selection hints do not bypass admission: the runtime remains authoritative
and can still reject a model when live memory is insufficient.
The HTTP server is loopback-only when configured locally; non-loopback
addresses are rejected unless `MARINA_HTTP_ALLOW_NON_LOOPBACK=true` is set for
an explicitly secured deployment. Network exposure is not enabled by default.
HTTP requests may provide `timeout_ms`; expiry cancels the generation and
returns a timeout response. Native GGUF output is detokenized through the
llama.cpp vocabulary rather than by rewriting token fragments. Requests may
provide `context_size`, bounded to 32,768 tokens in this first HTTP slice;
the runtime also rejects generation when available memory is unknown or when
the conservative model-plus-context reserve cannot be admitted. The response
contains a Marina-owned `generation_id` separate from the caller's request ID.
Chat Completions may set `chat_template_kwargs.enable_thinking` to a boolean;
the native GGUF adapter applies it to models whose chat template exposes that
option, including Qwen3.5. The C template formatter uses built-in formats,
so the adapter supplies the supported thinking prefill explicitly.
`TALVIK_HTTP_CONTEXT_SIZE` changes the default HTTP context from 4,096 tokens
(maximum 32,768); `TALVIK_ENABLE_THINKING=false` disables thinking by default
for templates that support it. Explicit request values override these defaults.
These daemon defaults also work with IDE clients that discard extra model
options. Advertise an IDE input-plus-output budget smaller than the configured
context to leave room for native chat-template tokens.
`TALVIK_HTTP_MAX_TOKENS` sets the default output budget (otherwise 128) when
an IDE omits both `max_tokens` and `max_completion_tokens`; explicit request
limits take precedence.
The current policy remains CPU-first and single-generation; accelerator-aware
allocation and multi-generation scheduling are follow-on capabilities.

This bridge supports model listing, streaming and non-streaming chat, function
tool calls, and inline image input when the selected model has a vision
projector. The token is an access credential for Marina; it is not a provider
API key. Remote BYOK provider credentials are not yet implemented and must
remain in the IDE or an OS secret manager. The bridge binds to loopback by
default and must not be exposed publicly without the planned authenticated
service boundary.

The provider-neutral OID client uses `OID_MARINA_URL`, `OID_MARINA_TOKEN`, and
optional `OID_MARINA_MODEL`. Its default endpoint is
`http://127.0.0.1:11435` on Windows, where port 11434 is reserved for local
IPC, and `http://127.0.0.1:11434` on Linux/macOS. The first client slice rejects
remote endpoints until a trusted TLS boundary is implemented.

The OID Console is a Linux-first MarinaOS component and uses Talvik's local
IPC protocol exclusively for model operations. It requires a running,
protocol-compatible daemon and does not fall back to an in-process runtime.
Talvik's HTTP and OpenAI-compatible endpoints are for external clients such as
assistants and applications; they are not an alternate OID Console backend.

External HTTP clients configure `OID_MARINA_URL`, `OID_MARINA_TOKEN`, and
optionally `OID_MARINA_MODEL` in their own client process. Keep HTTP bound to
loopback unless a trusted TLS, identity, authorization, and network-exposure
policy has been configured.

Daemon startup reserves its HTTP and local IPC listeners before starting the
runtime. A second process using the same listener exits with an address-in-use
error. A running Unix socket is preserved; only a stale socket is removed on
restart. Separate development instances require separate listener addresses
and state directories.

Concurrent HTTP clients and IDE utility requests share the runtime's bounded
single-generation queue. Admission and enqueue transitions are synchronized so
simultaneous requests cannot race for the active slot or miss queue wakeups.


### Terminal chat presentation

`talvik run <model-id> <prompt>`, `oid-marina-chat <prompt>`, and OID console
responses share a streaming terminal renderer:

- Cyan model/answer labels, muted thinking text, bold Markdown headings and
  emphasis, and cyan inline/fenced code. Markdown remains copyable as text;
  this is lightweight highlighting, not full Markdown layout or language syntax
  highlighting.
- An animated waiting indicator with elapsed time until tokens arrive. This
  does not claim to distinguish queueing, loading and prompt processing, which
  the IPC stream does not report separately.
- A Thinking section only for an emitted leading `<think>` block containing
  text. Empty wrappers stay hidden. Code containing literal thinking tags is
  preserved. Normal responses start with Generating.
- A compact completion footer with elapsed time and reported token metrics;
  cancellation and output limits have separate labels.
- Ctrl+C cancels while waiting for the first token as well as during streaming.
  Progress is cleared and styles reset on handled completion/error/cancellation.

Use `talvik --plain run ...` or `oid-marina-chat --plain ...` to get raw streamed
model text. `TALVIK_PLAIN=1` disables the renderer for all three clients.
Nonempty `NO_COLOR` disables colors while retaining readable interactive labels.
`TERM=dumb` and redirected output disable decoration and animations. Progress
and summaries use stderr; raw model text goes to stdout. `talvik run` no longer
appends the internal `DONE` protocol record to its output.

Development acceptance checks (Unix; use an explicit native library directory
for native-linked development binaries):

```sh
cargo test -p oid-console
TALVIK_TEST_NATIVE_LIB_DIR="$HOME/.marina/lib" python3 scripts/test-talvik-terminal.py target/debug
```

The terminal checks launch real clients in pseudo-terminals against a local
fixture daemon; they do not require models or modify the installed daemon.
Conventions: <https://clig.dev/#output> and <https://no-color.org/>.


### OID Console: dot prompt and compact status

Start the optional console with `oid-console`. The dBase-style `. ` prompt stays
constant when changing directories. Ordinary input still goes to the shell;
OID commands use their existing colon namespace: `:help`, `:status`, `:generate`,
and `:quit`. Use `pwd` to inspect the current directory.

The interactive footer reserves one bottom row for **state · model**, with a
`:help` hint only when there is room. There is no model inventory or telemetry
dump at startup, no repeated status lines in command output, and no extra block
cursor drawn into input. Runtime details remain available with `:status`,
`:hardware` and model/backend commands. `OID_STATUS_MODE=hidden` removes the
footer. The legacy `full` setting now retains the same compact footer rather
than restoring the automatic telemetry dump.

The console respects the terminal palette and `NO_COLOR`. Long command lines
use a single-row viewport; history, Unicode input, Home/End, Ctrl+A/E, Ctrl+L,
Ctrl+C and Ctrl+D are supported. Bracketed multiline paste becomes one editable
line rather than executing commands automatically. Native programs temporarily
regain the whole terminal; the footer returns after they exit. Resize, graceful
exit and handled signals restore terminal settings. Plain/dumb terminals and
pipes receive no footer/cursor escape sequences, including from `:clear`.

`OID_HISTORY_FILE` overrides the normal `~/.oip/history` path, including for
isolated development checks. The presentation stays in the client; model
ownership and the governed operation boundary remain in Talvik/OID services.

```sh
TALVIK_TEST_NATIVE_LIB_DIR="$HOME/.marina/lib" python3 scripts/test-oid-console-terminal.py target/debug
```

The Unix acceptance suite uses a fixture daemon and pseudo-terminals to check
footer layout, resize, shell handoff, Unicode, hidden/plain modes and terminal
restoration. Input and resize handling use [crossterm](https://docs.rs/crossterm/0.28.1/crossterm/event/index.html);
width calculations use [unicode-width](https://docs.rs/unicode-width/0.2.0/unicode_width/).
