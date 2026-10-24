# ADR-0010: OID-Owned Local Model Runner Boundary

- Status: Accepted

## Decision

OID owns a backend-neutral `ModelRunner` contract in `crates/model-runner`.
The first local compute implementation is llama.cpp, isolated in
`runtime/adapters/llama-cpp` and its opt-in FFI crate. Model lifecycle,
streaming, cooperative cancellation, statistics, and errors use OID-owned
types.

The dependency direction is:

```text
OID Console → Intelligent Runtime → ModelRunner → llama.cpp adapter → GGUF
```

The current slice supports multiple resident local GGUF models, one active
generation, and CPU-first execution. It includes streamed text generation,
cooperative cancellation, native embeddings, and model-template handling, but
does not yet provide concurrent scheduling, tool execution, RAG, chat/session
state, or distributed inference. ADR-0011 extends this boundary into the product strategy: the OID
API should feel Reference local runner-like for local users, while a future router selects
among adapters without making Reference local runner a control-plane dependency.

## Rationale

Reference local runner is a practical benchmark and interoperability target, not OID's
permanent runtime boundary. Direct adapter ownership preserves control over
hardware identity, lifecycle, observability, governance, and future routing.
The adapter can later be replaced without changing OID clients.

## Alternatives rejected

- Making Reference local runner the permanent runtime would hide lifecycle and execution
  details OID needs for governance and hardware-aware scheduling.
- Exposing llama.cpp throughout OID would couple clients to one engine and its
  ABI/version choices.
- Writing a custom inference engine now would expand the first proving slice
  before OID has trustworthy measurements.

## Current limitations

The native C shim is intentionally opt-in because the repository does not
vendor llama.cpp. Its FFI declarations must be validated against the exact
llama.cpp build used on each target. CPU-first live inference is qualified on
the recorded macOS x86_64 host; Linux, Windows, and macOS arm64 remain separate
platform gates.
