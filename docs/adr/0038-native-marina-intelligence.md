# ADR-0038: Native Marina Intelligence

- Status: Proposed
- Related: ADR-0015, ADR-0021, ADR-0027

## Context
Local inference is available through Talvik, while OIP is optional and may be
offline or unavailable.

## Decision
Marina Intelligence is a native provider-neutral service for understanding,
planning, context, explanation, and preference adaptation. Talvik, OIP, and
future adapters implement inference only; no provider gets OS authority.

## Alternatives and consequences
OIP-only operation creates an online dependency; embedding inference in the
control plane violates runtime separation. Provider adapters add contracts and
fallback testing but enable offline use. Private context requires retention and
disclosure policy.

## Migration, acceptance, open questions
Start with local Talvik and a typed proposal adapter. Accept when local mode
works with OIP disconnected and provider identity is observable. Open: memory
store implementation and model-selection policy.
