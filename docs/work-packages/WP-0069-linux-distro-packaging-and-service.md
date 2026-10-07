# WP-0069: Linux Distro Packaging and Service

## Purpose

Make Marina a trustworthy Linux product that follows distro conventions and
can run as a user or system service.

This is the final productization work package. It starts after the standalone
cross-platform runner, public API, routing/provider contract, and runtime
isolation work have reached their acceptance criteria; it must not block those
earlier milestones.

## Scope

- Debian/Ubuntu and Fedora/RHEL package prototypes, then a support matrix.
- FHS-aligned binaries, libraries, configuration, state, cache, models,
  runtime, and logs.
- Hardened systemd system and user units.
- `marina service` lifecycle commands with explicit user/system mode.
- Upgrade, rollback, uninstall, purge, model preservation, and migration.
- Offline package installation, signatures, checksums, dependencies, and
  architecture/accelerator variants.
- GPU/device access and backend-specific service permissions.

## Acceptance criteria

- A clean supported distro can install, start, stop, inspect, upgrade, rollback,
  and remove Marina without OID or a development checkout.
- User and system services have separate state, sockets, locks, identities, and
  model roots.
- The service runs unprivileged with tested systemd sandboxing and explicit
  device/network permissions.
- Package removal never deletes models without an explicit purge action.
- Logs, health, crash recovery, and exit status are diagnosable with standard
  distro tools.

## Dependencies

WP-0065, WP-0066, WP-0067, WP-0068, WP-0070, WP-0071, WP-0062;
ADR-0006, ADR-0012, ADR-0014.
