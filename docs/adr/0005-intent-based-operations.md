# ADR-0005: Intent-Based Operations

- Status: Accepted

## Context

Traditional shells expose programs and flags. The project aims to let users describe outcomes such as “why is nginx failing?” or “summarize today’s logs?” while preserving safe, explainable system actions.

## Decision

Console commands express user intent rather than directly invoking operating-system commands. The runtime and future operation layer translate intent into typed plans, policy decisions, approved actions, execution, verification, and audit records. Underlying tools such as `ls`, `find`, `systemctl`, or package managers remain implementation details of authorized operations.

## Consequences

The interaction model is more approachable and distinctive, but intent interpretation must be bounded by explicit capabilities, approvals, postcondition checks, and evidence. The command grammar requires careful handling of ambiguity, confirmation, errors, and escape hatches for literal shell usage.

## Alternatives Considered

- Direct shell passthrough is familiar but does not provide intent-level planning or consistent safety controls.
- A chatbot-only interface hides operational structure and is poorly suited to repeatable system work.
- A fixed command-only grammar limits natural intent and makes the console less useful for complex tasks.

## Future Considerations

The grammar should support both human-friendly intent and explicit command modes, structured arguments, dry runs, explainability, cancellation, and machine-readable plans.

