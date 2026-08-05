# WP-0020: Mock Runtime

## Purpose

Provide a deterministic runtime service so the console can validate its architecture and UX before a model backend exists.

## Scope

Mock `status`, `health`, `backend`, `models`, memory display, command lifecycle events, startup/shutdown events, and input lifecycle events. No AI, inference, llama.cpp, model loading, hardware probing, or Linux operations.

## Deliverables

- `RuntimeService` client-facing trait.
- `RuntimeSnapshot` value contract.
- `MockRuntime` implementation with deterministic placeholder data.
- Event publication for command and runtime lifecycle.
- Unit tests for values and event emission.

## Acceptance Criteria

- Health is `Healthy`, backend is `None`, models are `0`, and memory is `--`.
- Repeated calls return equivalent values.
- Commands emit a complete mock lifecycle without invoking external AI services.
- Console compiles against the interface rather than concrete backend types.

## Dependencies

WP-0002, WP-0003, WP-0018; ADR-0003, ADR-0004.

## Future Work

Replace or supplement the mock with a real runtime implementation behind the same interface and retain deterministic test doubles.

