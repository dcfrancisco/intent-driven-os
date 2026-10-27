# ADR-0026: Talvik Native Inference Performance and Worker Isolation

- Status: Accepted for incremental M2 implementation
- Date: 2026-10-24
- Related: ADR-0003, ADR-0006, ADR-0007, ADR-0010, ADR-0021, ADR-0025;
  WP-0027, WP-0031, WP-0034, WP-0058, WP-0062, WP-0091, WP-0094, WP-0098

## Context

A real Continue session reached Talvik and streamed Qwen output, but a short
request remained active, repeated Markdown fragments, and monopolized the
single generation slot. A captured production process was still CPU-active
more than one hour after the request began. An isolated Qwen request also had
high latency while competing with that production process. This proves client
connectivity, but not acceptable inference performance.

The local reference implementation was inspected at commit
`f52b9e742d8c9a308f7b23cd1f0ab52c7ddb694d`. The relevant source is
`server/sched.go`, `llm/llama_server.go`, and `api/types.go`. Talvik evidence is
in `runtime/src/service.rs`, `runtime/src/native_llama.rs`,
`runtime/adapters/llama-cpp/src/lib.rs`,
`runtime/adapters/llama-cpp/sys/src/lib.rs`, and `console/src/bin/http.rs`.

## Source-backed comparison

| Area | Talvik before this decision | Reference implementation | Decision |
|---|---|---|---|
| Model selection | Exact registered ID, explicit load, no silent substitution | Canonical names/manifests and keyed runner reuse | Preserve exact identity; add aliases only through WP-0097 |
| Residency | Multiple model handles in one daemon; FIFO one-active policy | Keyed loaded runners with references and expiry | Preserve Talvik scheduler; do not rebuild verified residency |
| Native execution | A new context, KV cache, sampler, and graph per request | Persistent per-model serving process and reusable prompt cache | Introduce a persistent per-model execution worker behind the existing backend boundary |
| Failure isolation | A native assertion aborts `talvikd` | Per-model serving process failure is recoverable by the scheduler | Move native generation to supervised workers before production qualification |
| Prompt evaluation | Entire formatted prompt evaluated on every request | Prompt caching enabled for repeated prefixes | Add bounded, model-owned context reuse with explicit invalidation |
| Batch sizing | Graph batch had been tied to requested context size | Bounded batch defaults and resource-aware limits | Bound physical batch independently from logical context |
| Output decoding | Entire generated token history was detokenized on every token | Incremental token pieces are streamed | Decode one new token piece at a time |
| Request options | HTTP ignored several standard sampling fields | Full option translation including stop controls | Map supported controls explicitly and reject unsupported controls when semantics matter |
| Stop/repetition | EOG plus hard-coded markers; no repetition guard | EOG, request stop strings, and bounded repeated-piece protection | Add request stop sequences and a model-neutral repetition safety guard in WP-0098 |
| Cancellation | Cooperative native abort in the daemon | Request-context cancellation at worker boundary | Retain cooperative cancellation and add worker termination/recovery fallback |
| Metrics | End-to-end latency and aggregate throughput | Load, prompt-eval, token-eval, and completion timings | Publish queue wait, prompt evaluation, first token, token generation, and termination reason |
| Native optimization | One locally linked library build | Packaged platform-specific optimized variants | Qualify compiler/CPU dispatch and packaged libraries per platform; never infer performance from compilation |

## Decision

Talvik keeps its backend-neutral scheduler and public contracts. Performance is
improved in two bounded stages.

Stage A is implemented in-process: use a physical batch of at most 512 tokens,
decode larger prompts in chunks, stream each token piece incrementally, map
`max_completion_tokens`, `temperature`, `top_p`, `top_k`, and `seed`, stop only
on complete terminal markers, and expose prompt/first-token timing.

Stage B is being implemented incrementally under WP-0098. The current slice
gives each resident model an owned persistent native context behind the existing
one-active-generation scheduler. Worker identity is still the exact Talvik
model ID. The scheduler retains ownership of admission,
pinning, FIFO fairness, eviction, authentication, cancellation, and public
model state. A worker crash fails only its request and model, never the daemon
or unrelated resident models.

Context reuse is conservative: cache keys include model digest, template,
tokenized prompt prefix, context size, and native build identity. Any mismatch,
cancelled mutation, failed decode, model unload, or worker restart invalidates
the cache. Talvik must remain correct with caching disabled.

The implemented slice reuses an exact formatted-token prefix by trimming the
native sequence KV state with `llama_memory_seq_rm` and re-evaluating the final
cached token to restore sampler state. It reports cache-hit and reused-token
counts in generation metrics. A mismatch in model context size, formatted token
prefix, cancellation, decode failure, unload, or restart drops the context.
This is persistent in-process context ownership, not crash-isolated process
workers; the latter remains open because a native assertion can still terminate
the host process.

## Invariants

1. Requested model IDs are never substituted.
2. An active model worker cannot be evicted or destroyed.
3. Context state is never shared across independent model IDs or incompatible
   request histories.
4. Cancellation, timeout, disconnect, and native failure release scheduler and
   memory reservations exactly once.
5. A worker crash cannot terminate `talvikd` or corrupt another model.
6. Logical context size is independent from physical decode batch size.
7. Unsupported generation controls are rejected or explicitly reported; they
   are not silently ignored.
8. Private prompts, responses, and credentials are not written to diagnostics.

## Alternatives rejected

- Copying the reference scheduler or making it a dependency: Talvik already
  has verified scheduling/residency behavior and must retain its Rust boundary.
- Raising timeouts or completion limits: this hides runaway generation and
  worsens slot starvation.
- Hard-coding Qwen or DeepSeek stop strings: termination must come from model
  metadata, the applied template, EOG tokens, or explicit request controls.
- Enabling concurrent generation first: persistent context ownership, memory
  accounting, and crash isolation are prerequisites to a safe concurrency
  decision under WP-0094.
- Keeping direct native FFI indefinitely: an assertion currently aborts the
  service, which is not an acceptable production failure boundary.

## Qualification

Stage A requires unit tests, strict Clippy, workspace regression, a real GGUF
prompt larger than the physical batch, valid SSE completion, and timing fields.
Stage B requires repeated-prefix and unrelated-prompt tests, model A/B
isolation, cancellation recovery, worker crash recovery, unload/reload, memory
pressure, and before/after TTFT and throughput measurements on each advertised
platform. The exact-prefix native Qwen test now verifies KV reuse and cache
invalidation cases; its single-host timing is evidence only, not a performance
claim. Interactive IDE evidence remains separate from HTTP compatibility.

## Consequences

The immediate changes reduce avoidable per-token work and prevent context-sized
graph batches. They remove repeated context construction for compatible
requests, while preserving a safe full-rebuild path for mismatches. Public
performance claims remain limited until WP-0098 completes process isolation and
uncontended native benchmarks.
