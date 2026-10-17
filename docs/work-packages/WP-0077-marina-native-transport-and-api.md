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

### Qualification transfer to WP-0078

WP-0078 owns the outstanding native Linux x86_64 qualification obligations
transferred from this WP. Its current machine-readable result is
`docs/qualification/linux-x86_64-wp-0078.json`. WP-0077 remains open until
Linux native execution is verified or the governance owner formally narrows
the cross-platform acceptance criteria. The macOS x86_64 local-only baseline
remains qualified and is not replaced by this transfer.

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

### Checkpoint 7: runtime reliability and release qualification

The native llama.cpp diagnostic was reproduced under controlled conditions on
macOS x86_64 using the Qwen2.5 0.5B Instruct Q4_K_M GGUF from
`$HOME/.marina/models`. The installed library reports the llama.cpp C API
library as `0.3.0` and ggml as `0.22.0`; the build was CPU-only with
`GGML_NATIVE=ON`, `GGML_METAL=OFF`, `GGML_OPENMP=OFF`, and `GGML_BLAS=OFF`
from commit `18443257a30c884d5332abb8e7dc43c7ffe42fda`. The model metadata
reported Qwen2 architecture, Q4_K medium quantization, and a 32,768-token
training context. Marina used contexts 128, 512, and 4096 with batch and
micro-batch bounded to 512.

Normal one-token inference succeeded at all three contexts. When cancellation
or a 1 ms timeout interrupted an in-flight decode, native stderr repeatedly
reported:

```text
ggml_backend_sched_graph_compute_async failed with error 1
process_ubatch: failed to compute graph, compute status: 1
llama_decode: failed to decode, ret = 2
```

The pinned upstream `llama.h` defines the context abort callback as causing
`llama_decode()` to abort, and defines `llama_decode()` return value `2` as
“aborted”; GGML status value `1` is `GGML_STATUS_ABORTED`. The adapter's abort
callback observes the cancellation flag, returns a cancelled result, frees the
native context, and clears the request's cancellation registry entry. The
daemon then accepts subsequent requests. This establishes that the observed
`llama_decode ret=2` is an intentional abort status, not an ordinary model or
allocation failure. The upstream graph logger still emits error-looking text
for the abort, so that diagnostic is retained as an upstream behavior risk.

The native wrapper now classifies `llama_decode` status `2` as cancellation
only when Marina cancellation is active. A status `2` without cancellation,
or any other nonzero status even when cancellation races with it, remains a
native failure. Regression tests cover these distinctions; no native error is
suppressed.

Authoritative pinned API references:
`https://github.com/ggml-org/llama.cpp/blob/18443257a30c884d5332abb8e7dc43c7ffe42fda/include/llama.h`.

The repeatable qualification script ran three success, three cancellation,
and three timeout/recovery sequences. Generation IDs were unique and
monotonic within the daemon (`marina-gen-33150-5` through `marina-gen-33150-16`,
with timeout requests consuming IDs). A second principal's self-cancel token
received HTTP 403 when attempting to cancel another principal's request; the
owner then cancelled it successfully with HTTP 202. No stale permit or
contradictory terminal HTTP result was observed. Durable OID evidence remains
covered by Checkpoint 5's restart test; direct HTTP qualification does not
create OID evidence by design.

The qualification script is `scripts/qualify-marina-http.sh`. macOS x86_64 is
qualified for local-only release with the upstream abort diagnostic documented
and correctly classified. macOS arm64, Linux, and Windows native runtime tests
were not available in this workspace and remain unverified. Remote-network
deployment is not release-qualified; its separate acceptance criteria are
recorded in `docs/MARINA-RELEASE-READINESS.md`.

### Checkpoint 7B: focused abort-path investigation

The corrected release was smoke-tested through success, cancellation,
timeout, and recovery. Success and recovery returned normal completed
responses; cancellation returned `finish_reason=cancelled` and HTTP 202 from
the cancellation endpoint; timeout returned HTTP 504. The native log retained
the upstream abort diagnostic, while the API outcome remained structured and
recoverable. The new FFI regression tests cover intentional abort, abort
without an active cancellation, and a concurrent non-abort native failure.

### Checkpoint 8: cross-platform qualification

No additional platform was qualified. The available host is macOS x86_64
(`Darwin 21.6.0`). Docker client support is installed, but the Docker daemon
is unavailable, so a Linux runtime could not be started. No Windows host,
Windows runner, or Apple Silicon host is attached. Linux x86_64, Windows
x86_64, and macOS arm64 are therefore blocked/unverified with explicit
prerequisites recorded in the compatibility matrix. Cross-compilation or
client-only availability is not treated as native runtime evidence.

`qualify-marina-http.sh` now supports `QUALIFY_REPORT` and emits the
machine-readable `marina.runtime.qualification/v1` JSON schema after a passing
run. WP-0078 has since qualified the native Linux x86_64 Phenom II baseline;
the Ubuntu GitHub Actions workflow remains a separate unverified baseline.
Existing macOS x86_64 behavior remains preserved. Local-only and remote-network
readiness remain separate decisions.

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
