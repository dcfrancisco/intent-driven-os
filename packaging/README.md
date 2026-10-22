# OID/Talvik image packaging

These files implement the first system-profile image composition described by
ADR-0023. Marina packaging remains governed by ADR-0014 and WP-0069.

WP-0089 packages these same assets into a versioned Debian artifact with
`scripts/package-oid-deb.sh`. It includes the `talvikd` daemon, `talvik` client,
and `oid-console` binaries plus relocatable native libraries; it does not
include model files. Run `scripts/qualify-oid-deb.sh` against the resulting
package to test dpkg lifecycle behavior in an isolated root.

Build the system package on native Linux from qualified artifacts:

```sh
./scripts/package-oid-deb.sh \
  --version 0.1.0 \
  --talvikd target/release/talvikd \
  --talvik target/release/talvik \
  --oid-console target/release/oid-console \
  --native-lib-dir /path/to/qualified/llama/lib
```

Install or upgrade on an authorized Debian/Ubuntu host with `dpkg -i`; then
explicitly enable/start the existing units with `systemctl`. Package removal
does not purge `/var/lib/marina/models`, `/var/lib/oid/evidence`, or conffiles.
The package does not bind or claim Ollama's port; Marina retains its documented
loopback default and can be configured separately.

For a rootless OID client installation, use `scripts/install-oid-user.sh`.
It installs only `oid-console`; Marina remains managed by
`scripts/install-model-runner.sh` under `$HOME/.marina`.

The two services intentionally have different identities:

- `talvikd.service` runs the model runner as `marina`, owns `/var/lib/marina`,
  and exposes the Unix socket at `/run/marina/marina.sock` plus loopback HTTP
  at `127.0.0.1:12434`.
- `oid-readiness.service` and `oid-console.service` run as `oid`. OID may
  request intelligence from Marina, but model output remains subject to OID's
  governed-operation pipeline.

The console is a TTY service, not a graphical desktop. A missing model or
missing client token is a degraded readiness state and does not prevent boot.
Provision `/etc/oid/marina-client.env` with an inference-scoped token after
installation, then restart `oid-readiness.service`. The model installer must
verify a SHA-256 before placing an artifact in the Marina model directory.

Install these files under their conventional `/usr/lib/systemd/system`,
`/usr/lib/sysusers.d`, `/usr/lib/tmpfiles.d`, `/etc/marina`, and `/etc/oid`
locations when building the image. The image builder performs this mapping.
