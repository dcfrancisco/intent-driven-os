# WP-0085: Marina CPU-Compatible Small-Model Training Backend

## Purpose

Deliver the first real Phase 1 training vertical slice using a bounded,
CPU-compatible learning backend.

## Scope

- Replaceable backend adapter for classification, routing, ranking, or
  structured prediction.
- Deterministic bounded training with resource limits and checkpoints.
- Candidate artifact registration and serving compatibility handoff.

## Acceptance criteria

- A real CPU training job consumes a registered dataset and produces a
  versioned unpromoted candidate.
- Training dependencies remain isolated from the core inference runtime.
- Active production inference remains unchanged during success, failure, and
  cancellation.
- Actual training tests are clearly separated from mocked lifecycle tests.

## Dependencies

WP-0082, WP-0083, WP-0084, ADR-0020, WP-0077.

## Status

Planned. Do not start until contracts and lifecycle isolation are accepted.
