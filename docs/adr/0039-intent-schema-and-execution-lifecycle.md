# ADR-0039: Intent schema and execution lifecycle

- Status: Proposed
- Related: ADR-0005, ADR-0009, ADR-0038

## Context
Plans from models are untrusted and existing lifecycle code must become the
MarinaOS contract.

## Decision
Require identity, authenticated requester, outcome, targets, capabilities, risk,
authority, steps, verification criteria, and constraints. Lifecycle is received,
interpreted, validated, planned, authorized/awaiting approval, queued, running,
verifying, and terminal (completed, failed, cancelled, or recovered).

## Alternatives and consequences
Raw shell text is rejected as the default. Typed plans improve auditability but
require capability schemas and adapters. Exactly-once Linux execution is not
claimed; leases, idempotency, and reconciliation are required.

## Migration, acceptance, open questions
The `IntentProposal` slice is the first contract. Accept with duplicate,
crash-recovery, cancellation, and verification tests. Open: versioned wire format.
