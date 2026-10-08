# WP-0088: OID Bootable Linux Image and Boot-to-Operation Proof

## Purpose

Implement the first reproducible Debian/Ubuntu-family bootable image with OID
and Marina managed as separate system components.

## Scope

- Pin a minimal Debian/Ubuntu base image and image build toolchain.
- Install Marina using the ADR-0014 system profile.
- Install OID policy, evidence, operation coordination, and console services.
- Define systemd ordering, readiness, restart, and failure behavior.
- Provide offline first boot with no cloud credentials.
- Verify model artifact checksum/provenance before activation.
- Persist protected OID evidence and preserve it across service restart.
- Demonstrate prompt → plan → authorization → execute → verify → evidence.
- Provide image build, boot, test, upgrade, rollback, and troubleshooting docs.

## Non-goals

- Custom kernel or new Linux distribution from scratch.
- Cloud BYOK, distributed training, or model pretraining.
- Autonomous production OS mutation.
- GPU-specific image variants in the first proof.

## Acceptance criteria

- A clean supported VM or target machine boots the image offline.
- Marina and OID start under separate unprivileged identities.
- OID verifies Marina readiness before exposing the console as ready.
- A local model is loaded through the controlled model lifecycle.
- A read-only governed system request completes with durable evidence.
- An unauthorized mutating request is blocked before execution.
- Marina failure is surfaced without fabricating successful OID execution.
- Restart preserves policy state and evidence without loading credentials into
  logs.
- The build and boot test are repeatable from documented inputs.

## Dependencies

ADR-0023, ADR-0014, ADR-0016, ADR-0017, ADR-0021, WP-0069, WP-0071, WP-0077,
and a qualified Marina Linux baseline from WP-0078.

## Status

In Progress. The first image composition, systemd units, service identities,
readiness/degraded-state helper, checksum-gated model installer, image
manifest, Linux image builder, and in-VM qualification script are implemented
as packaging assets. The builder preserves native-library symlinks, writes an
`/etc/fstab` entry for the image filesystem, emits a machine-readable input
and output manifest, and rejects unsafe model IDs. No bootable image has been
qualified yet: the current macOS workspace lacks the Linux image toolchain and
a runnable VM.

## Implemented assets

- `packaging/marina/marina.service` uses the ADR-0014 system profile:
  `/usr/bin/marina`, `/etc/marina`, `/var/lib/marina`, `/run/marina`, and
  loopback HTTP `127.0.0.1:12434`.
- `packaging/oid/oid-readiness.service` records `ready` or `degraded` without
  blocking offline boot when Marina or a model is unavailable.
- `packaging/oid/oid-console.service` starts the existing console on tty1 as a
  separate `oid` user with the explicit Marina HTTP backend.
- `scripts/install-oid-image-model.sh` requires model ID and SHA-256 and uses
  atomic placement under `/var/lib/marina/models`.
- `scripts/build-oid-linux-image.sh` builds a Debian bookworm/systemd BIOS raw
  image on Linux from already-qualified native Linux binaries.
- `scripts/qualify-oid-image.sh` is an in-VM gate; it does not turn packaging
  or compilation into boot qualification.

## Traceability and qualification boundary

ADR-0023 owns image composition, boot ordering, offline readiness, and the
boot-to-governed-operation proof. ADR-0014 remains authoritative for Marina's
FHS paths, native libraries, service identity, model state, and uninstall or
purge semantics. WP-0069 remains the packaging prerequisite; this WP does not
replace distro-native install/upgrade qualification.

The current repository evidence is limited to source inspection and shell
validation. A real Linux image build, VM boot, model inference, governed
operation, offline boot, reboot persistence, and service recovery remain
UNVERIFIED until executed on a Linux host or VM.

## Current verification

Passed: shell syntax checks and `git diff --check`.

Unverified: real image build, VM boot, native systemd startup, GGUF inference,
governed operation, offline boot, reboot evidence persistence, and restart
recovery. These remain mandatory before this WP can close.
