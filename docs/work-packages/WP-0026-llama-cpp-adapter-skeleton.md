# WP-0026: llama.cpp Adapter Skeleton

## Purpose

Validate that the first intended inference engine can be represented as an independent implementation of the common backend contract.

## Scope

Separate `oid-llama-cpp-adapter` crate, descriptor, lifecycle methods, health, model operations, streaming placeholder, cancellation, embeddings placeholder, and tool-support placeholder.

Explicitly excluded: llama.cpp dependency, FFI, GGUF parsing, tokenizer algorithms, tensor math, token generation, and model loading.

## Deliverables

- Adapter crate registered in the Cargo workspace.
- `LlamaCppAdapter` implementing `oid_runtime::Backend`.
- Deterministic placeholder responses.
- Contract conformance test.
- Adapter boundary documentation.

## Acceptance Criteria

- The adapter compiles without an inference engine dependency.
- The runtime does not depend on the adapter crate.
- No llama.cpp or GGUF implementation is present.
- Future engine work can be added without changing console APIs.

## Dependencies

WP-0022, WP-0023; ADR-0003, ADR-0008.

## Future Work

Integrate llama.cpp behind the contract after a separate implementation WP covering FFI safety, model lifecycle, cancellation, and operational testing.

