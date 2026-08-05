# WP-0009: Observability

## Purpose

Make runtime behavior explainable and operable through consistent logs, metrics, traces, health states, and audit records.

## Scope

Structured logging, metrics, tracing, audit events, correlation, privacy, retention, health monitoring, and failure visibility across registry, scheduler, adapters, API, and console.

## Deliverables

- Event taxonomy and correlation-ID rules.
- Metrics catalog for capacity, latency, tokens, memory, queues, errors, and lifecycle.
- Trace boundaries from client request through backend adapter.
- Append-only audit schema and redaction/retention policy.
- Health model for runtime, devices, models, and backends.

## Acceptance Criteria

- An operator can explain why a request was admitted, queued, rejected, or failed.
- Logs, metrics, traces, and audit events distinguish operational data from sensitive content.
- Health checks do not accidentally mutate runtime state.
- Crash and recovery events preserve causality across process boundaries.

## Dependencies

WP-0002, WP-0003, WP-0008, WP-0010; ADR-0006.

## Future Work

Add exporters, dashboards, SLOs, alerting, audit export, and enterprise telemetry integration.

