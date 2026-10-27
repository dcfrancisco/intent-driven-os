# WP-0104: User/System Execution Capability Contract

## Status

Proposed. This package defines architecture and contracts only; it does not
authorize daemon implementation, privilege changes, or a product rename.

## Purpose

Specify how a future Linux-first Marina/OID orchestrator requests governed
operations from separate user-scoped and system-scoped execution contexts
without giving model processes direct operating-system authority.

## Scope

- Define typed capability requests, results, errors, request identity, and
  versioning for executor boundaries.
- Map user/session capabilities to a possible `marina-usrd` boundary and
  privileged system capabilities to a possible `marina-sysd` boundary.
- Define caller identity, least privilege, policy decision placement, approval
  enforcement, and authorization checks at execution time.
- Specify how plans, approvals, execution, verification, rollback, and evidence
  correlate across process and privilege boundaries.
- Define executor health/readiness, timeout, cancellation, restart, duplicate
  request, and partial-failure semantics.
- Compare this proposal with existing `oid-operation-coordinator`,
  `oid-policy-engine`, `oid-linux-skills`, and evidence/verification contracts.
- Identify how the proposal relates to Talvik while keeping inference separate
  from OS execution authority.
- Record migration, installation, service-account, IPC, and compatibility
  decisions needed before implementation.

## Explicit non-goals

- Implementing `marina-sysd` or `marina-usrd`.
- Adding unrestricted command, shell, script, or arbitrary filesystem access
  to a privileged service.
- Moving policy or approval decisions into Talvik or model output.
- Renaming the repository, products, crates, binaries, APIs, or installation
  paths.
- Implementing specific network, package, mount, device, power, or desktop
  operations.

## Acceptance criteria

- The contract is backend- and transport-neutral and has no model-provider or
  native inference types.
- User and system principals, capability scopes, and privilege transitions
  are explicit and independently testable.
- The executor revalidates authorization and approval; a caller cannot elevate
  authority by editing a request or replaying an approval.
- Every mutation has a stable operation/request identity and a documented
  terminal result, verification outcome, rollback status, and evidence link.
- Talvik has no direct route to executor APIs and cannot invoke OS operations.
- Failure, cancellation, timeout, daemon restart, duplicate delivery, and
  unavailable evidence storage have explicit safe behavior.
- The contract's ownership model is reconciled with ADR-0005, ADR-0006,
  ADR-0016, ADR-0019, ADR-0023, and WP-0045 before implementation WPs are
  created.
- The MarinaOS brand/repository question is explicitly decided separately;
  this WP does not assume a rename.

## Dependencies

ADR-0005, ADR-0006, ADR-0016, ADR-0019, ADR-0023; WP-0045, WP-0049, WP-0053,
WP-0054, WP-0068, WP-0072, WP-0088.

## Deliverables

- Reviewed capability and principal model.
- Trust-boundary and request-lifecycle diagrams.
- Versioned request/result/error contract.
- Threat model and privilege-boundary analysis for the user/system split.
- Decision record identifying the authoritative orchestrator and policy
  owner.
- Follow-on implementation work packages, if accepted.

## Risks and open questions

- Whether the execution orchestrator remains OID-owned or becomes a
  Marina-branded shared service.
- How system service requests authenticate the unprivileged caller and carry
  approval evidence without making approval tokens reusable.
- How to keep `marina-sysd` small, auditable, and independently updateable.
- Whether a user daemon is necessary or whether existing user-session
  capabilities can remain in-process behind the same contract.
- How operation recovery behaves when one executor is unavailable or upgraded.
- Whether the MarinaOS umbrella and `marina` interface names pass public-name
  and repository/package availability review.
