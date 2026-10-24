# WP-0094: Talvik M2 Concurrent Inference Qualification

## Purpose

Evaluate, rather than assume, whether concurrent generation is safe and useful
for Talvik's llama.cpp context ownership and memory model.

## Scope

Measure one-active-generation and candidate parallel configurations for
throughput, time to first token, total latency, RAM, CPU, cancellation latency,
and model-load time. Implement concurrency only after resource reservations
and native context ownership are proven.

## Dependencies

WP-0058, WP-0091, WP-0093; ADR-0007, ADR-0011, ADR-0025.

## Acceptance criteria

- Representative native GGUF measurements exist for the supported platforms.
- No active native context can be invalidated by eviction or cancellation.
- A documented decision either keeps one active generation or implements a
  bounded, tested concurrency policy.

## Status

Deferred pending measurements; one active generation remains the M2 baseline.
