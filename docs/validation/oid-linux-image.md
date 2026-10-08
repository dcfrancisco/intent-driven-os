# OID Linux image build and qualification

WP-0088 uses a minimal Debian bookworm `amd64` base with systemd and a BIOS
raw disk for the first proof. This is an image-builder choice, not a new
Marina packaging profile: Marina follows ADR-0014 and WP-0069.

## Build prerequisites

Run on a disposable native Linux build host as root. The host must provide
`debootstrap`, `parted`, `losetup`, `mount`, `mkfs.ext4`, `rsync`, `chroot`,
`grub-install`, and `grub-mkconfig`. Docker is not required at runtime and is
not used by the image. The builder installs a kernel and systemd inside the
image; it does not modify the host OS.

Build native Linux Marina/OID binaries first using the WP-0078 procedure, then:

```sh
sudo scripts/build-oid-linux-image.sh \
  --marina target/release/marina \
  --marinactl target/release/marinactl \
  --oid-console target/release/oid-console \
  --llama-lib-dir /path/to/llama/lib \
  --output artifacts/oid-bookworm-amd64.raw
```

Optional model inclusion requires an exact SHA-256 and model ID. A model is
not required for boot; without one the readiness state is `degraded`.

The builder writes `artifacts/oid-bookworm-amd64.raw.sha256` and a
machine-readable `artifacts/oid-bookworm-amd64.raw.manifest` containing the
image, executable, native-library, and optional model hashes. Keep model
artifacts outside the source repository unless a user explicitly chooses an
image build input. `--llama-lib-dir` is the native Linux build output and is
installed under `/usr/lib/marina`; omitting it is only valid for a binary that
has no external native backend dependency.

## First boot provisioning

The image starts Marina on `/run/marina/marina.sock` and loopback HTTP
`127.0.0.1:12434`. It does not require cloud access. Provision an
inference-scoped token through a controlled local procedure, without putting it
in source control:

```sh
sudo install -o root -g oid -m 0640 /path/to/marina-client.env /etc/oid/marina-client.env
sudo systemctl restart oid-readiness.service
```

Install a separately verified model when it was not included in the image:

```sh
sudo /usr/libexec/install-oid-image-model \
  --file /media/model.gguf \
  --sha256 SHA256 \
  --model-id local-model
sudo systemctl restart marina.service oid-readiness.service
```

## VM qualification

Boot the raw image in a disposable QEMU VM on Linux, with a writable disk or
snapshot for evidence. Run:

```sh
qemu-system-x86_64 -m 4096 -smp 2 \
  -drive file=artifacts/oid-bookworm-amd64.raw,format=raw,if=virtio \
  -nographic
sudo /usr/local/sbin/qualify-oid-image /var/lib/oid/qualification/boot-report.json
```

The mandatory WP-0088 tests still require actual execution: systemd startup,
missing-model degraded boot, real GGUF inference, governed plan/authorization/
execution/verification, offline startup, service restart, and evidence
survival across reboot. `scripts/qualify-oid-image.sh` only checks boot and
service invariants; it is not a substitute for those end-to-end tests.

## Current status

The workspace has passed shell syntax and patch validation for these assets.
Image build and VM boot are **unverified** because the current macOS host does
not have the Linux image toolchain or a runnable QEMU VM. No WP-0088 release
claim is made until the Linux VM evidence is captured.
