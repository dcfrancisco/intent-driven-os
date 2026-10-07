# WP-0058: Verified Model Cache and Admission

## Purpose

Make model artifacts trustworthy and prevent requests from starting when
context, memory, device, policy, or concurrency constraints cannot be met.

## Scope

Manifest schema, SHA256 verification, cache layout and atomic writes, trust
states, model variant selection, memory/context reservations, and deterministic
admission failures. The first explicit local/HTTP(S) GGUF pull path is now
implemented by `ModelAcquirer`; policy, resumable transfers, and richer
manifests remain in this work package.

## Acceptance criteria

- Unverified or corrupt artifacts cannot load.
- Admission explains each rejection and releases reservations on failure or
  cancellation.
- Restart preserves registry/cache state or fails closed without corruption.
- The one-model runner remains compatible while the control plane gains a
  clean path to multi-model residency.

## Dependencies

WP-0024, WP-0025, WP-0029; ADR-0006, ADR-0007, ADR-0011.
