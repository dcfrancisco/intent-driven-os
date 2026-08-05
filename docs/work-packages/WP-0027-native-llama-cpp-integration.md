# WP-0027: Native llama.cpp Integration

## Purpose

Replace the contract-only adapter skeleton with a runtime-owned adapter that
links directly to the official llama.cpp C API.

## Scope

Process-wide initialization/shutdown, health, descriptor metadata, system/build
information, and safe ownership of native model handles. No generation or
streaming.

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

## Future Work

Generation, streaming, context management, and accelerator policy remain
separate work packages.
