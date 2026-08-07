# WP-0050: Recovery and Resume CLI Boundary

## Status

Complete — Milestone 5.

## Outcome

`OperationCoordinator::recoverable` exposes incomplete operation records for a
future terminal command such as `operations recover`. The API intentionally
separates recovery inspection from automatic resume so resumed execution can
require explicit user policy and approval.
