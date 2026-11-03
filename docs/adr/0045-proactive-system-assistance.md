# ADR-0045: Policy-bounded proactive system assistance

- Status: Proposed
- Related: ADR-0016, ADR-0043, ADR-0044

## Context
Maintenance assistance is useful but must not become silent autonomous mutation.

## Decision
Separate observe, recommend, and act. Act requires explicit current policy;
destructive or security changes require user authorization. Notifications have
budgets, suppression, audit, opt-out, and disclosure rules.

## Alternatives and consequences
Always-on action is unsafe; no proactive behavior misses failures. Bounded
recommendations provide value while preserving user control.

## Migration, acceptance, open questions
Start with low-disk/service-crash observations. Accept with opt-out, suppression,
budget, and no-external-transmission tests. Open: default notification policy.
