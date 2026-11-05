# ADR-0057: MarinaOS local control IPC

- Status: Proposed for M0 implementation
- Related: ADR-0041, ADR-0042, ADR-0043, ADR-0053, WP-0115

## Context

The operating-system foundation needs one authority for readiness and
diagnostics. A CLI that reads readiness files directly cannot provide a stable
control boundary or distinguish daemon absence from a healthy degraded system.

## Decision

marinaosd is the single unprivileged foundation daemon. It exposes a
versioned, read-only, newline-delimited JSON protocol on
/run/marinaos/marinaos.sock. Phase 1 methods are version, status, doctor, and
hardware. The socket is created with mode 0660 and owned by the marinaos
service identity; filesystem access is the initial local authentication
boundary. Mutating methods, shell execution, network IPC, and Talvik control
are out of scope.

marinaosctl remains the compatibility command name and becomes the client.
When the socket is unavailable it returns an actionable error and does not
silently report stale readiness.

## Consequences

The daemon owns the status contract and can later add peer-credential checks
without changing CLI commands. Packaging must install marinaosd in
marinaos-core and marinaosctl in marinaos-cli. The socket group/access policy
requires clean Ubuntu VM validation before release.
