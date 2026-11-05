# MarinaOS repository inventory

Inventory date: 2026-11-04

## Implemented and reusable

| Area | Repository location | Current truth |
| --- | --- | --- |
| Local inference runtime | runtime/, crates/model-runner/ | Existing Talvik/OID runtime; optional to MarinaOS boot |
| Hardware discovery | runtime/src/hardware.rs | Runtime-coupled discovery; useful reference, not the foundation daemon contract |
| Historical image | image/, scripts/build-oid-linux-image.sh | Debian Bookworm prototype; not Ubuntu-qualified |
| Foundation client | crates/marinaosctl/ | Implemented client for the read-only MarinaOS IPC methods |
| Foundation daemon | crates/marinaosd/ | Implemented unprivileged daemon with versioned local socket protocol |
| Ubuntu package inputs | packaging/marinaos/, packaging/sysusers/, packaging/tmpfiles/ | Source-present; requires Ubuntu package execution |
| Package build | scripts/package-marinaos-core-deb.sh | Produces separate marinaos-core and marinaos-cli DEBs with SHA-256 sidecars |
| Package lifecycle checks | scripts/qualify-marinaos-core-deb.sh | Existing isolated check requires update for the two-package layout |

## Compatibility and migration

The existing /var/lib/marina, ~/.marina, Talvik names, and OID package remain
compatibility surfaces. MarinaOS foundation state is /var/lib/marinaos and its
local socket is /run/marinaos/marinaos.sock. No migration deletes existing
data. Talvik remains separately installable and is never required by the
foundation daemon.

## Not implemented or not qualified

- Ubuntu 24.04 clean VM installation and systemd execution.
- Bootable Ubuntu image or ISO.
- Hardware qualification, including the Phenom X6.
- AppArmor/seccomp profiles and peer-credential authorization beyond socket
  filesystem permissions.
- Mutating IPC methods, update rollback, recovery UI, and desktop.
