# WP-0077: Marina Native Transport, API, and Authorization

## Purpose

Implement ADR-0021 so Marina owns the permanent service boundary used by OID,
IDEs, assistants, Athena Hub, Delivery Wizard, and future training jobs.

## Implementation order

1. Native HTTP server with versioned health, model, generation, stream, and
   cancellation resources.
2. Persistent listener configuration with Unix-socket defaults and explicit
   loopback TCP/HTTP opt-in.
3. Authentication, local principals, scopes, and authorization decisions.
4. Request IDs, streaming terminal events, cancellation, timeouts, queue and
   concurrency limits.
5. Minimal memory/context admission before OID integration.
6. OID remote client and conformance tests.
7. Multi-listener/multi-backend extension points.

## Acceptance criteria

- Marina serves its versioned API without the Python bridge in the request path.
- Linux/macOS Unix-socket defaults remain compatible.
- Windows loopback TCP remains configurable and local by default.
- HTTP binding is loopback-only unless explicit secure deployment policy is set.
- Authenticated clients receive only authorized operations and scopes.
- Requests have stable IDs, cancellation, timeout, terminal, and metrics
  semantics across transports.
- Resource admission rejects unsafe concurrent/context/memory requests.
- OID uses Marina through the public API without reading model files or owning
  backend handles.
- Existing IDE OpenAI-compatible behavior remains available during migration.
- Training work cannot start until this WP's API and admission criteria pass.

## Non-goals

- Direct LAN exposure.
- Training algorithms or training endpoints.
- Making OpenAI routes the internal Marina API.
- Removing the Python bridge before compatibility tests pass.

## Dependencies

WP-0057, WP-0058, WP-0059, WP-0066, WP-0067, WP-0068, WP-0071; ADR-0021.

## Status

In progress. The first native HTTP slice is implemented and verified on macOS
x86_64 from the installed Marina prefix. Evidence includes bearer rejection,
authenticated model discovery, model load, real GGUF chat completion, health,
and the same runtime service being used by the existing Unix socket. Native
llama.cpp detokenization now uses `llama_detokenize` over generated token IDs;
the rebuilt release produced correctly spaced text in a real HTTP completion.
HTTP request-body reads are bounded, and `timeout_ms` cancels an in-flight
generation using one absolute request deadline and returns a timeout response.
Oversized-body and context-limit regression tests cover the HTTP admission
edge. The native service assigns an independent `marina-gen-*` generation ID
while preserving the caller's request ID for correlation. Scoped bearer
authorization, nested listener configuration, and conservative
memory/context admission are implemented for the current single-listener
runtime. Linux/Windows target verification remains open.

The first OID client vertical slice is now available through the
provider-neutral `oid-model-client` crate and `oid-marina-chat` executable.
Against a live macOS x86_64 Marina daemon it discovered models and completed
real GGUF inference. Invalid credentials and an unavailable daemon produced
structured client failures without exposing the bearer token. The interactive
OID console now has an explicit `OID_RUNTIME_BACKEND=marina-http` selection
that preserves the local compatibility backend by default. A live smoke
client has been verified; interactive console cancellation/evidence behavior is
verified at Checkpoint 5, while target-platform verification remains open.

### Checkpoint 5: durable inference evidence and remote metrics

The existing append-only `FileEvidenceStore` now stores inference records in
the same `evidence.log`; no second audit or telemetry database was added. Each
request appends a privacy-preserving `started` record and one terminal record
linked to it. Records include OID session/request IDs, Marina generation ID
when known, model/backend, non-sensitive generation parameters, timestamps,
outcome, structured error class, verification state, and metrics. Raw prompts,
generated text, bearer tokens, and provider secrets are excluded. Reopening
the file in a new OID process and using `:inference inspect <request-id>`
retrieves the records.

The native HTTP response now carries an independent `generation_id`, `usage`,
and `metrics`; missing metric fields remain `unknown`:

| OID metric | Marina source | Meaning |
| --- | --- | --- |
| `prompt_tokens` | `usage.prompt_tokens` | Prompt tokens consumed |
| `generated_tokens` | `usage.completion_tokens` | Tokens emitted |
| `context_tokens` | `metrics.context_tokens` | Context tokens used |
| `tokens_per_second` | `metrics.tokens_per_second` | Output rate |
| `latency_ms` | `metrics.latency_ms` | End-to-end generation time |
| `inference_time_ms` | `metrics.inference_time_ms` | Backend inference time |

Verified on macOS x86_64 with native Marina and local Qwen GGUF: 4 prompt
tokens, 128 output tokens, 132 context tokens, 3.22 tokens/second, and
39,719 ms latency. Cancellation, unavailable-daemon, and invalid-auth
requests each produced terminal evidence; a second request after cancellation
completed successfully. Evidence was reopened after restarting OID. Full
Hardware-specific validation, Linux/Windows verification, and broader
multi-client scheduling remain open.

### Checkpoint 6: authorization, admission, and runtime reliability

Implemented in the native HTTP path:

- Bearer tokens carry explicit scopes: `models:read`, `inference`,
  `inference:cancel:self`, `inference:cancel:any`, and `models:admin`.
  Model discovery, inference, cancellation, and model administration are
  checked independently. Legacy two-field token records retain the default
  read/inference/self-cancel compatibility scopes.
- The HTTP server records the authenticated principal that admitted each
  request. A client can cancel its own request; cancelling another principal's
  request requires `inference:cancel:any`. The runtime still permits only one
  active generation, and terminal cleanup releases the permit.
- Admission requires known available system RAM and estimates
  `model_bytes + (context_tokens * 16 KiB) + 512 MiB` before generation.
  Unknown available memory, arithmetic overflow, and insufficient capacity are
  rejected. This is intentionally conservative and is not yet a complete
  accelerator/KV-cache accounting model.
- Configuration precedence is environment override, then nested YAML, then
  legacy flat YAML aliases. The supported nested TCP fields are
  `server.listeners[0].transport`, `bind_address`, and `port`; authentication
  defaults to enabled and non-loopback HTTP remains rejected without explicit
  deployment override.

Fresh native macOS x86_64 release verification used the installed GGUF model
under `$HOME/.marina/models`, an isolated token file, and loopback port 12435:

- scoped model discovery and real inference passed; an inference token was
  denied model administration with HTTP 403;
- request `cp6-cancel-1` returned generation `marina-gen-25163-2` after
  cancellation, and `cp6-recovery-1` returned generation
  `marina-gen-25163-3` immediately afterward;
- a `timeout_ms=1` request returned HTTP 504 and a subsequent request
  `cp6-timeout-recovery` succeeded with generation `marina-gen-25163-5`;
- invalid authentication returned HTTP 401 and an unavailable port failed
  without affecting the daemon.

The native CPU graph diagnostic was reproduced in the daemon's native stderr
during the controlled run: `ggml_backend_sched_graph_compute_async` returned
error 1, followed by `llama_decode ret=2`. The daemon remained usable and
subsequent requests recovered, but the root cause is not established. The
diagnostic is retained as a native-backend reliability risk; no fix is claimed.

Automated targeted tests pass for scoped identity behavior, nested config
flattening, unknown/insufficient memory admission, request limits, model
client mappings, evidence persistence, cancellation recovery, and console
integration. Linux and Windows runtime verification remain unperformed.

### Checkpoint 4B observed run

Verified on macOS x86_64 with a native-enabled release binary, the local
Qwen GGUF, and a PTY-backed `oid-console` session:

- `:models` discovered both Marina-managed GGUF models.
- A long `:generate` request was cancelled with Ctrl-C. OID displayed
  `Request console-69441-1 cancelled after 0 tokens` and returned to its prompt.
- A second request completed as `Request console-69441-2 completed`, proving the
  remote client and Marina active-generation permit recovered after
  cancellation.
- Invalid authentication and an unavailable Marina endpoint were exercised in
  separate interactive console sessions; each displayed a structured failure,
  preserved the prompt, and accepted `:quit` normally.
- The run used `OID_RUNTIME_BACKEND=marina-http` and the native daemon directly;
  `scripts/marina-openai-proxy.py` was not launched.
- Runtime generation lifecycle events were published for the remote request.
  A same-session `:inspect system` operation remained governed by OID and
  reported four append-only in-memory records: plan, authorization,
  execution, and verification. The isolated durable evidence file was not
  created by that read-only foundation path, and inference commands did not
  create governed-operation evidence. This remains an integration gap rather
  than an invented success claim.
