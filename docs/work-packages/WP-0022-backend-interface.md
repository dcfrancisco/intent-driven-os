# WP-0022: Backend Interface

## Purpose

Define the backend-neutral contract that allows the Intelligent Runtime to orchestrate local and remote inference engines without knowing their implementation.

## Scope

Initialization, shutdown, health, model listing, model load/unload, streaming generation, cancellation, placeholder embeddings, and placeholder tool support. No backend implementation or inference.

## Deliverables

- `Backend` trait and stable descriptor types.
- Generation request and stream chunk contracts.
- Embeddings and tool-support placeholders.
- Structured health and lifecycle error boundary.
- Capability and compatibility guidance for llama.cpp, vLLM, Ollama, Docker Model Runner, and OpenAI-compatible providers.

## Acceptance Criteria

- The runtime orchestrator depends only on the common interface.
- No backend-native handles, parsers, tensors, or provider SDK types cross the boundary.
- Streaming and cancellation are represented without requiring a specific engine.
- A contract-only llama.cpp adapter compiles against the interface.

## Dependencies

WP-0003, WP-0008, WP-0018; ADR-0003, ADR-0008.

## Future Work

Add capability negotiation, token accounting, structured provider errors, process isolation, and real backend implementations.

