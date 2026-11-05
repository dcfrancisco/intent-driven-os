# ADR-0053: MarinaOS foundation is independent of Talvik

- Status: Proposed for review
- Related: ADR-0050, ADR-0051, ADR-0052, ADR-0056, ADR-0057, WP-0114, WP-0115

## Context

The first MarinaOS proof is an operable Ubuntu-based operating-system
foundation. Talvik is useful local inference, but it is a separately
installable runtime and must not become a boot dependency or a source of
privileged host authority.

## Decision

Ship matching amd64 DEBs: marinaos-core contains the unprivileged marinaosd
foundation daemon and service assets; marinaos-cli contains the marinaosctl
client. The daemon owns foundation readiness and persistent foundation state
through a read-only local IPC contract. Talvik is neither bundled nor required.

The first qualification baseline is Ubuntu 24.04 LTS amd64. The package
builder emits a SHA-256 sidecar and the CI workflow retains both the DEB and
machine-readable lifecycle evidence. Package removal does not delete
/var/lib/marinaos; purge semantics are intentionally deferred until a
separate data-retention decision.

## Security and consequences

The foundation service runs as marinaos with systemd hardening, no network
address families, and explicit read/write paths. Hardware reporting is
CPU-first and reports GPU/NPU as not qualified until telemetry-backed tests
exist. This creates a clean installable OS boundary, but does not by itself
provide a bootable image, installer, desktop, or hardware qualification.
