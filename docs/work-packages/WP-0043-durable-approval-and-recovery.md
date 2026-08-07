# WP-0043: Durable Approval and Recovery

## Status

Complete — Milestone 4.

## Outcome

`oid-policy-engine` now provides an append-only `FileApprovalStore` and the
`ApprovalLifecycle` contract. Pending, approved, consumed, and rejected states
are journaled and recovery derives the latest pending or approved operations
after a process restart.

Approval consumption is explicit and cannot succeed without an approved record.
The journal is separate from evidence so policy state remains independently
testable and replaceable.
