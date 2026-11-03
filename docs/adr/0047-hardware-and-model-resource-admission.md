# ADR-0047: Hardware and model resource admission

- Status: Proposed
- Related: ADR-0007, ADR-0025, ADR-0029, WP-0091

## Context
Machines vary widely and unsupported models must fail safely.

## Decision
Normalize CPU, memory, architecture, SIMD, accelerator availability, and
runtime-selected device. Admit models against measured memory/context budgets;
report CPU fallback explicitly and never infer GPU use from build capability.

## Alternatives and consequences
Optimistic loading improves convenience but causes crashes and misleading status.
Conservative admission can reject viable models, so model/resource explanations
are required.

## Migration, acceptance, open questions
Retain hardware service and Talvik admission. Accept legacy CPU, qualified GPU,
unsupported-model, and truthful-report tests. Open: AMD/Intel qualification.
