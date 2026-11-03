# ADR-0049: Delivery OS execution interoperability

- Status: Proposed
- Related: ADR-0037, WP-0104

## Context
Delivery OS may need isolated local execution but owns delivery objectives and
delivery-specific approval.

## Decision
Delivery OS requests bounded, authenticated execution environments through
MarinaOS capabilities. MarinaOS supplies policy-constrained workers, telemetry,
storage, and lifecycle; it does not inherit Delivery OS planning authority.

## Alternatives and consequences
Shared orchestration creates cyclic ownership. A narrow request/result contract
keeps platforms independently useful but requires identity and quota mapping.

## Migration, acceptance, open questions
Document contract before integration. Accept authorization, isolation, cancellation,
resource, and evidence-correlation tests. Open: transport and principal mapping.
