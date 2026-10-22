# WP-0089: OID/Marina Packaged Linux Environment

## Purpose

Deliver the first versioned Debian/Ubuntu system-profile artifact before the
bootable-image work in WP-0088. This package composes the already-qualified
Marina Linux binary, relocatable llama.cpp libraries, OID console, existing
systemd units, and existing configuration templates.

## Existing capability inventory

- `scripts/install-model-runner.sh`: user-scoped Talvik daemon/client installer;
  preserves `$HOME/.marina` state and models.
- `scripts/install-oid-user.sh`: new rootless OID console installer; it does
  not duplicate Marina installation and shares only the canonical native
  library/model home when explicitly supplied.
- `scripts/install-model-runner.ps1`: Windows user-scoped installer.
- `scripts/build-oid-linux-image.sh`: image-only composition, not a Debian
  package or general upgrade/uninstall mechanism.
- `packaging/marina/*`, `packaging/oid/*`, `packaging/sysusers/*`, and
  `packaging/tmpfiles/*`: existing system-profile assets reused here.
- `scripts/package-oid-deb.sh`: new versioned `oid-marina` Debian artifact.
- `scripts/qualify-oid-deb.sh`: new isolated dpkg install/upgrade/failure/
  removal data-retention test; it never changes host services.

There is no separate `oid-core` executable in the current workspace. The
current OID core/session boundary is delivered by `oid-console`; this package
does not invent or claim a nonexistent binary.

## Acceptance criteria

- Build `oid-marina_<version>_amd64.deb` from native Linux binaries and pinned
  relocatable `libllama`/`libggml` libraries.
- Include `talvikd`, `talvik`, `oid-console`, systemd units, sysusers/tmpfiles,
  configuration templates, and readiness helpers.
- Preserve `/var/lib/marina/models`, `/var/lib/oid/evidence`, and conffiles on
  package removal.
- Qualify package contents and dpkg lifecycle in an isolated filesystem.
- Record package checksum and target ABI metadata.
- Do not modify host services or the Phenom installation during qualification.
- Rootless users can install `oid-console` separately while keeping Marina's
  user-scoped lifecycle independent.

## Non-goals

Bootable image work (WP-0088), model bundling by default, cloud BYOK, training,
GPU variants, and remote network exposure.

## Status

In Progress. Native Linux Docker execution produced `oid-marina_0.1.0_amd64.deb`
and `oid-marina_0.1.1_amd64.deb`. Isolated dpkg qualification passed install,
upgrade, rejected invalid upgrade, removal, and retention of models, evidence,
and configuration. The package checksum for 0.1.0 is
`7ac0f8439e1bafd34001cd12c628b66db9df762c1cde92b2378854553381248c`.

Not yet qualified: starting/restarting the service under a real systemd boot,
GGUF inference from the installed package, ABI compatibility on each supported
Ubuntu/Debian release, and a real-host install. Static unit inspection in a
minimal extracted root reports missing distro target units, so it is not being
counted as systemd qualification. No real host installation has been performed.

## Traceability

Implements ADR-0014 and supports ADR-0023. Depends on WP-0065, WP-0069,
WP-0077, and WP-0078. WP-0088 remains deferred until this packaged
environment is operationally qualified.
