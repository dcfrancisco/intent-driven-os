# WP-0038: Operation Planning Framework

## Status

Complete — Milestone 3.

## Purpose

Standardize the lifecycle used by Linux skills, dynamic CLI capabilities, and
intent execution: plan, approve, execute, verify, rollback, and record evidence.

## Deliverables

- Canonical `OperationPlan` and `OperationStep` models.
- Risk and approval requirement enums.
- Verification and rollback plan models.
- Typed approved-plan, execution, verification, and rollback results.
- Deterministic plan JSON serialization.
- Skill lifecycle methods for planning, execution, verification, and rollback.
- Standard terminal rendering for inspectable operation plans.
- Existing system-health and create-directory flows migrated to the model.

## Acceptance criteria

- Mutating skills cannot execute from a raw request; they require an approved plan.
- Verification checks are declared before execution.
- Rollback is declared in the plan when supported.
- Evidence records include the operation plan identity and lifecycle categories.
- Workspace formatting, Clippy, tests, and doctests pass.

## Dependencies

WP-0010, WP-0018, WP-0037, and the existing skill, policy, verification, and evidence contracts.

