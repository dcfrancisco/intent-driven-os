# WP-0112: MarinaOS Linux Foundation

## Purpose

Deliver the first independently installable MarinaOS operating-system
foundation package without coupling boot, diagnostics, or persistent state to
Talvik or the historical OID package.

## Scope

- Ubuntu 24.04 LTS amd64 qualification baseline.
- marinaos-core DEB with marinaosctl, systemd, sysusers, tmpfiles, and
  persistent configuration/state.
- CPU-first hardware and capability reporting with explicit unqualified
  accelerator results.
- Offline diagnostics, readiness, restart/recovery behavior, checksums, and
  isolated dpkg install/removal evidence.
- GitHub Actions package artifact retention.

## Evidence contract

UNIT_TESTED means cargo test -p marinaosctl passed.
BUILT means the CI workflow produced marinaos-core_<version>_amd64.deb and
its SHA-256 sidecar. VM_TESTED and HARDWARE_TESTED require execution on the
stated Ubuntu VM or hardware and are not inferred from package contents or CI
compilation.

The qualification script records package hash, out-of-scope runtime absence,
and state retention after removal. It does not claim a real systemd boot or
clean-VM install when those tests have not run.

## Status

SOURCE_PRESENT / UNIT_TESTED locally. CI package creation and isolated dpkg
lifecycle are defined but have not been executed in this workspace. Ubuntu VM
boot, real service start/restart, upgrade, invalid-configuration diagnostics
on the installed package, and hardware testing remain NOT TESTED.

## Dependencies and next work

Depends on ADR-0053 and the pinned Rust toolchain. The next implementation
package is WP-0113, Ubuntu clean-VM installation and systemd lifecycle
qualification. Bootable image work remains blocked on that gate.
