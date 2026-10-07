# WP-0027: Native llama.cpp Integration

## Purpose

Replace the contract-only adapter skeleton with a runtime-owned adapter that
links directly to the official llama.cpp C API.

## Scope

Process-wide initialization/shutdown, health, descriptor metadata, system/build
information, safe ownership of native model handles, generation, streaming,
cancellation, and basic metrics.

## Deliverables

- `LlamaCppAdapter` implementing `Backend`.
- Isolated FFI/link crate with opt-in `LLAMA_CPP_LIB_DIR` configuration.
- Deterministic unavailable behavior when the native library is absent.

## Acceptance Criteria

- No subprocess or `llama-cli` invocation.
- No native pointer crosses the runtime API.
- Core workspace builds without a local llama.cpp installation.
- Configured builds link `libllama` directly.

## Dependencies

WP-0022, WP-0023, WP-0026; ADR-0003, ADR-0008.

## Status

Lifecycle, GGUF loading, streaming generation, cooperative cancellation, and
basic metrics have been implemented and validated in the retained native
acceptance evidence at `docs/reviews/model-runner-reality-review.md`. The
implementation remains CPU-first and one-model; verified cache/admission,
scheduling, and routing are separate in WP-0058 through WP-0060.
