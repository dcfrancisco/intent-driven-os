# ADR-0046: Talvik package and inference integration

- Status: Proposed
- Related: ADR-0012, ADR-0013, ADR-0014, ADR-0025

## Context
Talvik already has standalone binaries, native boundaries, packages, and
model/runtime state.

## Decision
Keep Talvik independently installable as daemon/client packages. MarinaOS
invokes its versioned backend-neutral contract. Evaluate `talvikd`, `talvik-cli`,
and optional integration packages; do not split without evidence.

## Alternatives and consequences
Embedding inference in MarinaOS couples release and hardware concerns. Separate
packages preserve cross-platform use but require version compatibility and health
checks. CPU/GPU claims must reflect actual execution.

## Migration, acceptance, open questions
Qualify existing DEB/RPM/archive workflows on clean Linux and preserve state on
upgrade. Accept install, real inference, health, fallback, and uninstall tests.
Open: final package naming and repository signing service.
