# ADR-0042: Policy authority and privilege separation

- Status: Proposed
- Related: ADR-0006, ADR-0016, ADR-0027, WP-0104

## Context
AI assistance must not become unrestricted root access.

## Decision
Authority derives from authenticated OS principals and policy, not model text.
Classify operations as read-only, reversible user, privileged reversible,
destructive, security-sensitive, or irreversible. Revalidate approval at the
execution boundary and use native Linux authorization mechanisms.

## Alternatives and consequences
Root daemon simplifies integration but creates catastrophic blast radius. A
narrow user/system boundary adds IPC and packaging work while limiting risk.
Secrets never enter model context unless explicitly authorized.

## Migration, acceptance, open questions
Keep current policy engine; define principal/scope contracts before privileged
services. Accept with unauthorized, approval, replay, and privilege tests. Open:
whether a separate system executor is needed.
