# ADR-0036: MarinaOS product identity and intent-driven architecture

- Status: Proposed
- Related: ADR-0019, ADR-0027, ADR-0023; `docs/MARINAOS-PRODUCT-ARCHITECTURE.md`

## Context
The repository has OID control-plane foundations and Talvik runtime foundations,
but the desired product story is one Linux AI operating system.

## Decision
Propose MarinaOS as the Linux-based product umbrella. It translates intent into
authorized, supervised, verifiable operations. Linux remains the security and
process authority; Talvik remains a separately packaged inference runtime.

## Alternatives and consequences
Keeping two OS brands preserves history but confuses ownership; renaming now
would break compatibility. The proposal therefore changes architecture language
first and defers executable renames. Security impact is positive only if model
output remains non-authoritative. Compatibility is preserved by existing names.

## Migration, acceptance, open questions
Use the migration inventory and first diagnostic slice. Accept after product,
repository, package, and trademark ownership review. Open: whether OID remains
the public desktop name beneath MarinaOS.
