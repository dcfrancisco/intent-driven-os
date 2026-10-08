# WP-0086: Marina Training Evaluation and Promotion Gates

## Purpose

Evaluate candidate models against explicit baselines and govern activation.

## Scope

- Reproducible evaluation suites and task-specific metrics.
- Baseline comparison, regression thresholds, schema/compatibility checks,
  latency and memory measurements.
- Approval records, candidate promotion, activation, rollback, and evidence.

## Acceptance criteria

- A completed training job remains unpromoted until evaluation and approval.
- Rejected candidates leave the active model unchanged.
- Approved activation and rollback are atomic or safely recoverable.
- Evaluation evidence identifies model, dataset, backend, configuration, and
  baseline hashes.

## Dependencies

WP-0084, WP-0085, WP-0087, ADR-0020, WP-0077.

## Status

Planned. No automatic promotion.
