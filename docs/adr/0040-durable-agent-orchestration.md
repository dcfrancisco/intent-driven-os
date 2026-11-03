# ADR-0040: Durable agent orchestration

- Status: Proposed
- Related: ADR-0009, ADR-0027, WP-0045, WP-0046

## Context
The operation coordinator already journals plans, approvals, execution, and
recovery; independent agents for every capability would duplicate state.

## Decision
Use one durable coordinator with logical roles (diagnostics, packages, files,
network, storage, processes, security, backup, hardware). Delegate typed work
with dependencies, bounded concurrency, cancellation, leases, retry policy, and
execution receipts.

## Alternatives and consequences
Per-capability daemons increase isolation but multiply trust and upgrade paths.
Logical roles preserve the current implementation and leave process isolation
available for a justified capability. No model process is an executor.

## Migration, acceptance, open questions
Map existing skills first; add process boundaries only after threat analysis.
Accept with restart, duplicate, stall, and cancellation tests. Open: cross-process
executor authentication.
