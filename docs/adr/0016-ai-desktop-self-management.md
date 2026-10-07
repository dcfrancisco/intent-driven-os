# ADR-0016: AI Desktop Self-Management Boundary

- Status: Accepted
- Date: 2026-10-07
- Related: ADR-0004, ADR-0006, ADR-0013, ADR-0015; WP-0068, WP-0072

## Decision

OID is an AI desktop/OS control plane, not only a model runner. A user may
prompt the computer in natural language, but OID must translate that request
into an inspectable plan, apply policy, request approval when required, execute
through bounded skills, verify the result, and retain evidence.

Marina supplies model capabilities. OID owns computer agency:

```text
User prompt -> context/state inspection -> intent and plan
  -> policy/risk/approval -> bounded desktop and OS skills
  -> verification/recovery -> evidence, explanation, and operating state
```

Self-management means the system can detect health, configuration, resource,
and maintenance conditions; propose or perform policy-approved remediation; and
verify the result. It does not mean unrestricted autonomous shell access,
silent privilege escalation, irreversible changes without approval, or model
responses directly executing tools.

## Agency levels

- **Observe:** collect read-only system, desktop, model, and health facts.
- **Recommend:** produce a plan with impact, prerequisites, rollback, and
  confidence for user review.
- **Auto-maintain:** execute allowlisted, reversible, low-risk actions under a
  policy profile and record evidence.
- **Approve-to-act:** pause for privilege, data loss, network, identity,
  package, security, or externally visible changes.
- **Recover:** stop, rollback, quarantine, or degrade when verification fails.

Every action carries a principal, scope, risk class, policy version, approval
record, timeout, resource budget, verification contract, and evidence ID.

OID system services and the interactive desktop are clients of the same OID
policy and evidence plane. Marina remains the shared local model service and
never receives authority to operate the computer directly. Skills own concrete
OS integrations; the model proposes actions as typed data.

## Consequences

The model runner is necessary but insufficient for the AI desktop product.
Future work must cover desktop context, skills, package/configuration
maintenance, permission brokering, notifications, recovery, user profiles,
offline behavior, and evaluation of action safety. “The model answered” is not
an operation success signal; verification is required.

This ADR defines the product direction. Autonomous desktop management,
durable system agents, broad hardware control, and self-updating behavior are
not yet implemented.
