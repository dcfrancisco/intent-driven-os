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

The current slice supports one active local GGUF model and CPU-first execution.
It does not train models, download models, or implement scheduling, tools,
RAG, or distributed inference.

## Rationale

Ollama is a practical benchmark and interoperability target, not OID's
permanent runtime boundary. Direct adapter ownership preserves control over
hardware identity, lifecycle, observability, governance, and future routing.
The adapter can later be replaced without changing OID clients.

## Alternatives rejected

- Making Ollama the permanent runtime would hide lifecycle and execution
  details OID needs for governance and hardware-aware scheduling.
- Exposing llama.cpp throughout OID would couple clients to one engine and its
  ABI/version choices.
- Writing a custom inference engine now would expand the first proving slice
  before OID has trustworthy measurements.

## Current limitations

The native C shim is intentionally opt-in because the repository does not
vendor llama.cpp. Its FFI declarations must be validated against the exact
llama.cpp build used on the Ubuntu target. The current host is macOS and has no
configured native llama.cpp library, so live inference and performance claims
remain pending Linux validation.
