# ADR 0009: Canonical operation planning framework

- Status: Accepted
- Date: 2026-08-07

## Context

OID now has separate intent, policy, skill, verification, rollback, and
evidence boundaries. Without a canonical plan, each capability could invent
its own approval semantics, verification behavior, and user presentation.

## Decision

All governed operations will produce an `OperationPlan` before execution. The
plan declares the operation identity, intent context, owning skill, rationale,
risk, approval requirement, ordered steps, verification checks, and rollback
steps. Execution accepts only an `ApprovedOperationPlan`; verification and
rollback consume typed execution results.

Plans are inspectable and expose deterministic JSON serialization without
requiring a serialization dependency in the core contract crate.

## Consequences

Every Linux skill, dynamic CLI capability, and future intent executor shares
one lifecycle and terminal presentation. Approval and postconditions become
reviewable before side effects occur. Skills must pay the up-front cost of
planning and declaring rollback behavior, and plan schemas must remain
backward-compatible as capabilities expand.

Intent data is embedded as an `IntentContext` snapshot in the common crate to
avoid a dependency cycle between the common contracts and intent runtime.

