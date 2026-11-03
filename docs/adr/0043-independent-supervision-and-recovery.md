# ADR-0043: Independent supervision and recovery

- Status: Proposed
- Related: ADR-0006, ADR-0023, WP-0056, WP-0062

## Context
Service liveness does not prove task progress, and a watchdog must not escalate.

## Decision
Supervisor observes service health, progress, stalls, resource exhaustion, retry
loops, deadlocks, and inference availability. It may pause/cancel/restart/escalate
within policy and records recovery state without granting authority.

## Alternatives and consequences
Process watchdogs alone miss logical stalls; policy-aware supervision adds state
and evidence. Recovery may produce an unverified state rather than a false success.

## Migration, acceptance, open questions
Extend operation journal and runtime health; accept crash, stall, cancellation,
and reconciliation tests. Open: watchdog placement on image boot.
