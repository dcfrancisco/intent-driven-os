# WP-0029: Model Loader

## Purpose

Prove that the runtime can own a real native model lifecycle.

## Scope

One loaded model, loading/unloading/loaded/failed states, backend-reported
memory, registry status, and lifecycle events.

## Deliverables

- Runtime model load/unload service methods.
- `LoadedModel` backend result.
- Model lifecycle event mapping.

## Acceptance Criteria

The console can request `model load <id>` and `model unload <id>`; a second
model is rejected while one is loaded; failures are observable.

## Dependencies

WP-0023, WP-0024, WP-0027, WP-0028.

## Future Work

Eviction, memory budgets, idle unloading, multiple loaded models, and GPU
scheduling.
