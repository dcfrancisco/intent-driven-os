# WP-0003: Backend Adapter Interface

## Purpose

Define the common contract through which the runtime discovers, enables, loads, runs, streams, stops, unloads, and monitors inference backends.

## Scope

Capability reporting, model compatibility, lifecycle calls, streaming, cancellation, health, structured errors, versioning, and adapter isolation. Initial compatibility target: `llama.cpp`; no adapter implementation.

## Deliverables

- Interface design and capability matrix.
- Backend-neutral request, response, stream-event, and error semantics.
- Adapter lifecycle and crash boundary.
- Version and compatibility policy.
- Mapping notes for future vLLM, ONNX Runtime, OpenVINO, TensorRT-LLM, Ollama, and Docker Model Runner adapters.

## Acceptance Criteria

- The contract does not expose backend-native handles or assumptions.
- Streaming, cancellation, backpressure, and disconnect behavior are defined.
- Unsupported capabilities are represented explicitly.
- The design can describe `llama.cpp` without constraining future adapters.

## Dependencies

WP-0002; ADR-0003, ADR-0008.

## Future Work

Implement adapters only after the contract is reviewed; evaluate process isolation and plugin packaging.

