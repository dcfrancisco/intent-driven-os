# ADR-0050: Ubuntu-based distribution and image strategy

- Status: Proposed
- Related: ADR-0014, ADR-0023, WP-0088, WP-0089

## Context
The product must become installable, but an installer rewrite is not the first
proof.

## Decision
Start with installable packages on supported Ubuntu LTS, then build a pinned,
reproducible image and qualify boot in a VM before installer/hardware work.
Support headless and desktop operation where practical; use normal Linux/systemd
security boundaries.

## Alternatives and consequences
A custom distro first delays useful behavior. Ubuntu packages accelerate proof
but inherit base-distro lifecycle and hardware constraints.

## Migration, acceptance, open questions
Use existing packaging/image assets. Accept clean install, offline startup,
upgrade, boot, degraded AI, and recovery tests. Open: Ubuntu release and desktop.
