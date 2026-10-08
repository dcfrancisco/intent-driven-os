# ADR-0023: OID Linux Distribution, Boot Lifecycle, and System Services

- Status: Proposed
- Date: 2026-10-09
- Related: ADR-0014, ADR-0016, ADR-0017, ADR-0021; WP-0069, WP-0071, WP-0088

## Context

OID is intended to become a Linux-first AI desktop/OS, not merely a console
installed on an existing desktop. ADR-0014 already defines Marina's FHS paths,
systemd units, user/system installation modes, and package lifecycle. This ADR
defines the OID bootable-image and operating-environment boundary without
duplicating Marina's packaging decision.

The first proof should use a minimal Debian/Ubuntu-derived image rather than
requiring a new Linux distribution from scratch. The image must boot offline,
start OID and Marina under explicit service identities, and demonstrate a
governed system operation from prompt through verification and evidence.

## Decision

OID's first bootable image SHALL be a reproducible Debian/Ubuntu-family image
with OID and Marina installed as managed system components. The image build
must be reproducible from pinned base-image, package, binary, and native
backend inputs.

The boot lifecycle SHALL have these boundaries:

1. The operating system initializes networking and local storage according to
   normal distro policy.
2. Marina starts as an unprivileged service and exposes only its configured
   local transports.
3. OID policy, evidence, and operation-coordination services initialize.
4. OID verifies Marina health, model availability, and evidence storage before
   declaring the AI operating environment ready.
5. The console becomes available only after readiness checks pass.

OID SHALL remain the authority for intent interpretation, planning,
authorization, execution, verification, and recovery. Marina SHALL provide
model execution and resource management but SHALL NOT receive unrestricted OS
authority from the image.

## Required image properties

- Offline startup after installation, without cloud provider credentials.
- Explicit systemd service ordering and readiness checks.
- Separate Marina and OID service identities and state ownership.
- Persistent evidence on a protected writable volume.
- Controlled model installation with checksum/provenance verification.
- No model prompt or response retention by default.
- Recovery after Marina failure without falsely reporting a successful OID
  operation.
- A repeatable boot-to-governed-operation test.
- Upgrade, rollback, and image/artifact provenance records.

The first image SHALL support CPU-only Marina operation. GPU and accelerator
variants are later matrix entries, not prerequisites for the first proof.

## Scope boundaries

ADR-0014 remains authoritative for Marina packages, FHS paths, native library
payloads, systemd hardening, user/system mode, and model artifact lifecycle.
This ADR owns the OID image composition, boot readiness contract, service
ordering, offline behavior, and system-level acceptance test.

This ADR does not authorize training execution, cloud BYOK, distributed
training, autonomous OS mutation, or a custom kernel/distribution build.

## Initial implementation work package

WP-0088 SHALL produce the first bootable-image proof and its reproducible boot
and governed-operation validation. Its implementation assets include the
first image manifest, Linux image builder, separate Marina/OID systemd units,
service users, readiness/degraded-state reporting, and checksum-gated model
import. It may use a VM or removable image during
development, but the acceptance evidence must identify the actual boot
environment and must not confuse container startup with OS boot qualification.

## Consequences

The project can prove the AI operating-environment experience before investing
in a custom distribution. Marina remains independently installable and
cross-platform, while OID gains a Linux-specific product delivery track.

The image introduces service ordering, state migration, offline model
provenance, and recovery obligations. Those are intentionally explicit rather
than hidden inside the interactive console.

## Implementation traceability

The current implementation assets are `scripts/build-oid-linux-image.sh`,
`scripts/qualify-oid-image.sh`, the `packaging/` service/user/path files, and
`docs/validation/oid-linux-image.md`. They establish reproducible image
composition and an evidence-oriented in-VM gate, but they do not constitute
boot qualification. WP-0088 remains In Progress until a real Linux VM or
machine executes the mandatory acceptance tests.
