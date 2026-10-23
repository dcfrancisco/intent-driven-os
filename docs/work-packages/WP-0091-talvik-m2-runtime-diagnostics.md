# WP-0091: Talvik M2 Runtime Diagnostics and Residency Edge

## Purpose

Expose accurate resident-model and scheduler state to external clients without
leaking backend-native handles or prompt content.

## Scope

- Add runtime-owned resident model summaries and pending/active counts.
- Add justified Reference local runner-compatible `/api/tags` and `/api/ps` responses.
- Preserve authenticated `/v1/*` behavior and loopback defaults.
- Add deterministic API contract tests and real-GGUF inspection evidence.

## Dependencies

WP-0058, WP-0061, WP-0062, WP-0067; ADR-0025.

## Acceptance criteria

- `/api/tags` lists registered models with truthful status metadata.
- `/api/ps` lists only actually resident models and measured/configured memory.
- Pending and active counts are observable without prompt/response content.
- Authentication and scope failures remain structured and tested.
- Two resident GGUF models can be inspected before and after unload/eviction.

## Status

Implementation slice complete on `feature/talvik-m2-runtime-ide`; deterministic
runtime/API tests pass. Memory admission now reports incremental physical
requirements and aggregate budget requirements separately, while queued
request cancellation removes the request before dispatch. Native two-model
endpoint qualification and external client qualification remain open
acceptance work.
