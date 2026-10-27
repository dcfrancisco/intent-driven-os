# ADR-0026: Inference Performance Measurement

- Status: Accepted
- Date: 2026-10-27
- Related: ADR-0003, ADR-0013, ADR-0021; WP-0034, WP-0057, WP-0059, WP-0067,
  WP-0068, WP-0092

## Context

Talvik can complete real-model inference and returns some generation metrics,
but a successful response alone does not establish responsiveness, throughput,
or where request time is spent. OID's HTTP client currently receives prompt
and output token counts and may receive latency, inference time, and
tokens/second. It does not consistently expose time to first token, prompt
evaluation time, generation duration, or model-load duration.

The standalone `oid-marina-chat` binary is the smallest OID-to-Talvik client
for validating this boundary. It currently prints only response text. A
process-level stopwatch is useful for user-observed elapsed time, but it
includes process startup, model discovery, HTTP transport, server work, and
output; it must not be presented as backend inference time or used to
misrepresent backend token throughput.

Performance evidence is also needed to distinguish a first request from warm
requests. “Cold” must be defined and reported carefully: a new client process
is not necessarily a cold model if `talvikd` and its model remain resident.

## Decision

### Metric ownership and semantics

Talvik is authoritative for runtime/backend measurements it can observe.
Every metric is optional, unit-defined, and attributed to its measurement
source. Missing server metrics remain `unknown`; clients must not fabricate
them.

The measurement contract should distinguish:

| Metric | Definition | Primary source |
| --- | --- | --- |
| Client-observed total latency | Monotonic elapsed time for client configuration, model discovery, and one generation request; excludes process startup and output rendering | OID client |
| Time to first token (TTFT) | Time from backend generation start to the first emitted output token | Talvik/backend |
| Prompt evaluation time | Time spent evaluating prompt tokens before generation | Talvik/backend |
| Generation time | Backend time spent producing output tokens, excluding prompt evaluation where supported | Talvik/backend |
| Prompt tokens | Tokens evaluated for the request | Talvik/backend |
| Output tokens | Tokens emitted by the backend | Talvik/backend |
| Generation throughput | Output tokens divided by backend generation time | Talvik/backend |
| Model load time | Talvik-observed duration around the model-load operation, reported only when the model was not resident at the preceding status check | Talvik |

If a backend only supplies aggregate inference duration, preserve that field
under its documented meaning. Do not relabel it as generation-only time. The
current native adapter exposes TTFT, aggregate inference duration, and its
backend throughput; prompt-evaluation and decode-only durations remain
unavailable. The HTTP API carries these as optional `metrics` fields, alongside
the prompt/output token counts. Model residency is an optional `resident`
field in `/v1/models`, so older API implementations remain valid and produce
an unknown benchmark state.

If throughput is not supplied by Talvik, OID must report it as unavailable;
server-reported throughput is never overwritten by wall-clock arithmetic.

### CLI experience

Extend `oid-marina-chat` with:

- `--timing <prompt>` to print the response plus total client latency and all
  server metrics that are available. Each field identifies its source or is
  shown as unknown.
- `--benchmark <prompt>` to run a bounded number of requests and report
  per-request results plus median and p95 client-observed latency and
  server-reported output throughput when available.

Benchmarking defaults to five requests and accepts `--runs N` for 1 through
100 requests. The latency interval covers each chat request only; per-run
model-residency discovery is outside that interval. P95 uses nearest-rank
selection. Responses are not printed or persisted in benchmark mode.

The benchmark must state its run count, model ID, backend, and whether each
sample was warm, first-after-load, or unknown. It must not restart `talvikd`,
unload a user's model, or claim a cold-start measurement without an observed
daemon/model-load boundary. A fresh client process with a resident model is
not a cold model start. Model load time is reported separately from request
latency.

The existing no-flag invocation remains compatible and prints response text
without requiring metrics from older or alternate API implementations.

### Release requirement

The MarinaOS/Talvik v0.1 release gate should require a real local-model
qualification record containing client total latency, backend throughput and
token counts when supported, TTFT and prompt/generation phase timings when
supported, model-load timing, and a documented first-request versus warm-run
methodology. Unsupported measurements must be explicitly identified rather
than fabricated.

## Scope boundary

This ADR covers performance instrumentation, metric semantics, client display,
and bounded benchmarking. It does not claim that every backend can report
phase-level timings and does not change model output semantics.

Preventing visible reasoning markers such as `</think>` and ensuring
instruction-following output are separate prompt/template/output-format
regression concerns. They must be tracked and tested independently, not
silently treated as performance work.

## Consequences

Users can distinguish perceived response time from backend inference
performance and identify missing measurements. Talvik may need to extend its
versioned response/stream metrics contract, while OID and the standalone chat
client must preserve source and units through rendering.

Benchmark comparisons require controlled model, backend, hardware, context,
sampling options, and warm/cold state. Numbers from different conditions must
not be presented as directly comparable.

## WP-0092 implementation notes

`oid-marina-chat --timing` includes OID client configuration, one model
discovery call, and the chat request. `--benchmark` reports only chat-request
latency and classifies a sample as first-after-load only when Talvik reports
that a load occurred during that request. The current native backend does not
report prompt-evaluation or decode-only duration, and those values remain
unavailable rather than being inferred from aggregate inference time.

## Implementation work package

WP-0092 SHALL implement the bounded CLI timing and benchmark surface and close
any required Talvik metric-contract gaps. It depends on WP-0034, WP-0057,
WP-0059, and the authenticated OID/Talvik integration in WP-0068.
