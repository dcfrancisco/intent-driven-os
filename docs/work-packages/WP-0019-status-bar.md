# WP-0019: Status Bar

## Purpose

Give the console a permanent, compact readout of runtime health and resource state.

## Scope

Runtime health, backend, model count, memory display, and current local time. Values are supplied by the mock runtime; no hardware probing or model registry is required.

## Deliverables

- Status-bar layout and update policy.
- `Runtime`, `Backend`, `Models`, `Memory`, and `Time` fields.
- Deterministic rendering seam for tests.
- Refresh behavior after commands and input cancellation.

## Acceptance Criteria

- The status bar displays `Healthy`, `None`, `0`, `--`, and current local time in the reference client.
- Status rendering does not perform hardware detection or inference.
- Placeholder data is supplied through runtime interfaces rather than hard-coded in the renderer.
- The bar does not corrupt prompt editing or command output.

## Dependencies

WP-0016, WP-0018, WP-0020; ADR-0004, ADR-0007.

## Future Work

Add terminal-width adaptation, live memory/accounting data, backend/model summaries, and cursor-presence integration.

