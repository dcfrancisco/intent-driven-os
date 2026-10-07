# WP-0057: Local Runner Contract Hardening

## Purpose

Turn the current llama.cpp-backed slice into a stable, testable local runner
contract without leaking native engine details.

## Scope

Request IDs, model manifests, capability descriptors, lifecycle invariants,
stream terminal semantics, cancellation, timeout/error taxonomy, and metrics.
Keep one active GGUF model and one active generation in this WP.

## Acceptance criteria

- Contract tests cover list, inspect, load, generate, stream, cancel, unload,
  unavailable backend, malformed request, and missing model.
- Every stream has exactly one terminal outcome and a stable request ID.
- Native handles/types remain inside the adapter.
- Linux validation records cold/warm load, time-to-first-token, throughput,
  cancellation latency, and memory for the acceptance model.

## Dependencies

WP-0022, WP-0027, WP-0031, WP-0032, WP-0034, WP-0035; ADR-0010, ADR-0011.

## Implementation status (2026-10-07)

The first executable slice is implemented: stable runtime request IDs, one
active model/generation invariants, streaming terminal events, native-backed
cooperative cancellation, and generation statistics. The remaining acceptance
gap is native Linux/macOS/Windows smoke evidence and resource/context admission.
