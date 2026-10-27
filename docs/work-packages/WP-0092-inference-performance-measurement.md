# WP-0092: Inference Performance Measurement

## Status

Implementation complete; real-GGUF release qualification remains pending.
Unsupported backends do not have to expose phase-level timings.

## Purpose

Make OID-to-Talvik inference performance observable without confusing total
client wait time with backend inference time.

## Scope

- Extend `oid-marina-chat` with `--timing <prompt>` while preserving the
  existing no-flag invocation.
- Add a bounded `--benchmark <prompt>` mode with a documented default run
  count of five and an explicit `--runs N` selector bounded to 1-100.
- Measure total client-operation latency with a monotonic clock and define
  which client steps it includes. Timing mode includes client configuration,
  model discovery, and generation; benchmark samples time only each generation
  request, excluding model-residency discovery.
- Propagate available Talvik metrics and units without replacing unknown
  values with estimates.
- Extend the Talvik API/streaming metric contract where needed for:
  - time to first token;
  - prompt evaluation time;
  - generation duration;
  - prompt and output token counts;
  - backend-reported generation throughput;
  - model load duration.
- Attribute each displayed value as Talvik/backend-reported,
  OID-client-measured, or unavailable.
- Report benchmark run count, model, backend, median and p95 observed
  end-to-end latency, and available server-reported output throughput.
- Distinguish warm requests, first request after observed model load, and
  unknown state. Do not equate process startup with model cold-start.
- Keep backend aggregate inference time distinct from generation-only time;
  leave prompt-evaluation and decode-only duration unavailable when unsupported.
- Provide a reproducible real-GGUF qualification procedure recording model,
  hardware, backend version, context/options, load state, and run methodology.

## Non-goals

- Automatically restarting `talvikd`, unloading models, or disrupting a
  running user's service to manufacture cold-start conditions.
- Claiming prompt-evaluation or generation-phase times when Talvik/backend
  does not measure them.
- Replacing server-reported throughput with CLI wall-clock estimates.
- Changing model prompts, chat templates, response semantics, or filtering
  reasoning markers such as `</think>`.
- Adding a long-running benchmark service, dashboard, or metrics exporter.

## Acceptance criteria

- `oid-marina-chat "<prompt>"` remains backward-compatible and prints the
  response as before.
- `oid-marina-chat --timing "<prompt>"` prints response text, client-observed
  total latency, and each available Talvik metric with units and source.
- Missing metrics are displayed as unknown/unavailable and never as zero.
- Backend throughput, if supplied, is preserved and displayed as backend
  throughput; client wall-clock latency is not used to overwrite it.
- `oid-marina-chat --benchmark "<prompt>"` performs a finite, configurable
  number of requests and reports median and p95 client-observed latency.
- Benchmark output identifies the model/backend, run count, and cold/warm/
  unknown classification; it never labels a fresh CLI process cold if the
  model was already resident.
- Measurement tests cover missing metrics, server-reported metrics, elapsed
  time boundaries, percentile calculation, and benchmark state labeling.
- A real-GGUF qualification run records backend metrics and client latency
  separately before the release gate is considered satisfied.
- No prompts, responses, or bearer credentials are persisted by benchmark
  output unless a separate explicit opt-in is designed.

## Implemented command and metric contract

```text
oid-marina-chat "<prompt>"
oid-marina-chat --timing "<prompt>"
oid-marina-chat --benchmark [--runs N] "<prompt>"
```

The HTTP response includes optional TTFT, prompt-evaluation duration,
aggregate inference duration, decode-only generation duration, model-load
duration, and backend identity.
The native adapter currently provides TTFT, aggregate inference duration, and
backend throughput; prompt evaluation and decode-only duration remain
unavailable. Talvik measures model-load duration around its load operation and
reports it only when the model was observed non-resident immediately before
loading. `/v1/models` reports optional residency, preserving compatibility
with servers that omit it.

Benchmark latency uses per-request monotonic wall time and reports the median
and nearest-rank p95. Each run prints observed token counts and backend
throughput when present. It queries model residency before each sample without
including that discovery call in the sample latency. Benchmark mode does not
print or persist prompts or generated responses.

## Real-GGUF qualification procedure

1. Record the model ID, artifact checksum, backend/library version, hardware,
   operating system, context size, and generation options.
2. With the model already resident, run
   `oid-marina-chat --benchmark --runs 5 "<fixed prompt>"`; record each sample
   state, OID latency, and available Talvik metrics.
3. If an operator has independently unloaded the model, run one
   `--timing` request to observe the load boundary, then repeat the benchmark
   for warm samples. Do not unload or restart a running user's service solely
   to manufacture a cold measurement.
4. Mark unsupported TTFT, prompt-evaluation, generation-only, throughput, or
   load metrics as unavailable; do not substitute client latency or estimates.
5. Keep the resulting qualification record with release evidence; the record
   is not produced automatically by the CLI.

## Dependencies

ADR-0026; WP-0034, WP-0057, WP-0059, WP-0067, WP-0068.

## Separate follow-up

Add a distinct chat-template/output-format regression test for unwanted
reasoning markers such as `</think>` and instructions that require a concise
single-value response. It is intentionally not an acceptance criterion for
this performance package.
