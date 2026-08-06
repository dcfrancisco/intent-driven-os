# WP-0034: Runtime Metrics

## Purpose

Make generation performance visible and testable.

## Scope

Prompt tokens, generated tokens, throughput, latency, inference time, and
context usage.

## Deliverables

- `GenerationStatistics`.
- Console summary output.
- Status-bar metric fields.

## Acceptance Criteria

Completed requests report deterministic metric fields without exposing backend
internals.

## Dependencies

WP-0031, WP-0032.

## Future Work

Aggregated metrics, histograms, Prometheus export, and per-backend dashboards.
