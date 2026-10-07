# ADR-0014: Linux Distribution and Service Integration

- Status: Accepted
- Date: 2026-10-07
- Related: ADR-0006, ADR-0007, ADR-0012, ADR-0013; WP-0065, WP-0069, WP-0071

## Context

Marina is a Linux-first product even though its standalone runner is
cross-platform. A shell installer that places binaries and mutable state in an
arbitrary prefix is not sufficient for a distro-quality Linux service. User
installation, system installation, model data, native backends, logs,
permissions, upgrades, and service supervision need different ownership and
lifecycle rules.

This ADR is intentionally sequenced as the final delivery phase. Portable
runner behavior, the versioned service API, routing/provider contracts, and
resource isolation must stabilize first. Linux distro packaging is a delivery
target, not a prerequisite for proving the standalone Marina product.

## Decision

Marina supports two explicit Linux installation modes.

The supported installation profiles are normative:

| Profile | Executables | Models | State/config/runtime | Supervisor |
| --- | --- | --- | --- | --- |
| Development | repository `target/` only | explicit test path | temporary/test paths | foreground |
| Portable user | `$HOME/.marina/bin` | `$HOME/.marina/models` | `$HOME/.marina/{state,run,logs}` | manual or user service |
| Distro user | `$HOME/.local/bin` | `$HOME/.marina/models` | XDG config/state/cache/runtime | `systemd --user` |
| Distro system | `/usr/bin`, `/usr/lib/marina` | `/var/lib/marina/models` | `/etc`, `/var/lib`, `/var/cache`, `/run` | systemd system unit |
| Rootless container | image entrypoint | mounted model volume | mounted state/runtime volume | container supervisor |

`MARINA_HOME`, `MARINA_MODEL_DIR`, `MARINA_STATE_DIR`, and the XDG variables
may override defaults only within the selected profile. The resolver must
report the effective profile and paths. A system service never silently falls
back to a user's home; a user service never silently adopts system state.

### User mode

User mode requires no root privilege and is the default for the development
installer:

```text
$HOME/.marina/bin/       executable entry points
$HOME/.marina/lib/       optional native backends
$HOME/.marina/models/    model artifacts and manifests
$HOME/.marina/state/     registry, locks, and migrations
$HOME/.marina/logs/      fallback logs when journald is unavailable
$HOME/.marina/run/       fallback socket and runtime metadata
```

The distro user service uses `$XDG_RUNTIME_DIR/marina/marina.sock` and is
managed by `systemctl --user`, with `loginctl enable-linger` an explicit opt-in
when the service must survive logout. The portable installer may use the
`.marina/run` fallback when no user runtime directory exists. A user service
never requires access to another user’s home or models.

### System mode

System mode is provided by distro packages and requires explicit elevation:

```text
/usr/bin/marina /usr/bin/marinactl
/usr/lib/marina/             backend libraries and helpers
/etc/marina/                  administrator configuration
/var/lib/marina/              registry and model state
/var/cache/marina/            re-creatable downloads/cache
/run/marina/                  runtime socket and service state
```

System mode runs under a dedicated unprivileged `marina` service account. The
system service does not read user homes by default. Model roots, GPU device
access, network access, and remote credentials are explicit configuration.

The system socket is `/run/marina/marina.sock`, owned by the service account
and a documented access group with mode `0660`. The daemon validates Unix peer
credentials (`SO_PEERCRED` or the platform equivalent) and maps the peer to a
Marina principal before accepting model-management or generation requests.
There is no unauthenticated `/tmp` socket fallback.

### Service lifecycle

Linux packages ship a hardened systemd unit and, where supported, a user-unit
template. `marina service install`, `start`, `stop`, `restart`, `status`, and
`uninstall` are idempotent and report whether they operate in user or system
mode. Package upgrades preserve models, registry state, configuration, and
audit history; uninstall removes binaries and service metadata but never model
artifacts unless an explicit purge operation is requested.

The systemd unit must use least privilege appropriate to the selected backend:
`NoNewPrivileges`, a read-only system view, private temporary space, restricted
address families, explicit writable paths, resource limits, restart limits,
timeout behavior, and explicit GPU device permissions. Sandboxing is tested
per backend because device access and memory behavior vary across CPU, CUDA,
ROCm, Vulkan, Metal, and other engines. The unit uses systemd-managed
`RuntimeDirectory`, `StateDirectory`, `CacheDirectory`, and `LogsDirectory`
where available rather than hand-creating ambiguous paths.

### Linux integration rules

- Logs go to journald in packaged service mode; file logs are fallback/opt-in.
- Socket ownership and permissions are explicit. A local socket is not
  equivalent to authorization; peer identity and request policy are required.
- User and system services do not share a registry or model lock by accident.
- Native libraries are package-managed or verified artifacts, never loaded from
  the current working directory.
- Release binaries and native backends have signed metadata, checksums,
  provenance/SBOM, supported-key rotation, and signature-failure behavior.
- Model artifacts have independent trust state from binaries and native
  backends. A checksum-less remote download may be cached for inspection but
  is not loadable under a verified-production policy.
- Model downloads use temporary files, verification, quotas, resumability, and
  cleanup; interrupted downloads never become loadable models.
- Packages provide architecture/accelerator variants and declare runtime
  dependencies instead of probing and silently installing system software.
- Install, upgrade, rollback, and removal are tested on supported distro
  families before a release is called Linux-ready.

## Linux release baseline

The first distro release targets Debian/Ubuntu and Fedora/RHEL-family x86_64
CPU installations with systemd. ARM64 is a separately tested architecture;
CUDA, ROCm, Vulkan, and other accelerators are separate variants with explicit
device/package/sandbox matrices. A distribution or architecture is not
advertised as supported merely because the Rust binary compiles there.

Before Linux-ready status, the release must demonstrate clean install,
user/system service start, socket authorization, model import/verification,
health and metrics, crash/restart recovery, upgrade/rollback, uninstall versus
purge, offline installation, and package signature verification.

## Known gaps promoted to release blockers

The current runner does not yet satisfy this ADR. The following are blocking
gaps rather than optional enhancements:

- no distro packages or service units;
- no profile-aware path resolver or system/user separation;
- no authenticated/versioned transport (the current tab protocol is local
  development-only);
- no journald integration, service health contract, or crash recovery;
- no signed release/native-backend/model provenance policy;
- no GPU/device permission or sandbox matrix;
- no upgrade migration, rollback, uninstall/purge, or offline-bundle tests.

## Consequences

The simple `$HOME/.marina` layout remains the standalone user experience, but
Linux distro packages use FHS-aligned system paths. A single hard-coded path is
not a valid cross-installation contract. Service installation is a product
feature with privilege, ownership, recovery, and upgrade semantics.

This ADR does not claim that systemd units, distro packages, sandbox profiles,
or GPU-specific permissions are implemented yet.
