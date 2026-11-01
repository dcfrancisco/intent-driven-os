# OID/Talvik image packaging

## Standalone Talvik packages

Standalone package builders are separate from the OID image package:

```sh
./scripts/package-talvik-deb.sh \
  --version 0.1.0 \
  --talvikd target/release/talvikd \
  --talvik target/release/talvik \
  --native-lib-dir /path/to/llama/lib

./scripts/package-talvik-rpm.sh \
  --version 0.1.0 \
  --talvikd target/release/talvikd \
  --talvik target/release/talvik \
  --native-lib-dir /path/to/llama/lib
```

Both builders fail closed when release binaries or native libraries are
missing. They do not download models, start services, or remove `.marina`
state. The user systemd unit stores models and registry data under the
invoking user's `$HOME/.marina`; enabling it remains an explicit operator
action with `systemctl --user enable --now talvikd.service`.

The tagged Talvik release workflow publishes both the Debian package and the
RPM package alongside the Linux, macOS Intel, macOS Apple Silicon, and Windows
archives. The Linux packages install the binaries, native libraries, default
model metadata, and the per-user systemd unit; they do not enable or start the
daemon as root. Enable the service for the logged-in user with:

```sh
systemctl --user daemon-reload
systemctl --user enable --now talvikd.service
```

The macOS archives contain `install-talvik-macos.sh`. Run it from the
extracted archive with `--service` to install a per-user launchd definition:

```sh
./install-talvik-macos.sh --service
```

The macOS helper does not require a privileged `.pkg`, does not modify global
paths, and preserves models and state during uninstall. With `--service`, installation starts one per-user daemon and enables startup
at login. The loopback OpenAI base URL is `http://127.0.0.1:11435/v1`; health
and readiness are `/healthz` and `/readyz`. Create a scoped client token with
`~/.marina/bin/talvik auth token create vscode`. The service profile reserves
a 32K context and a 2,048-token output budget; memory admission still applies.
It does not claim GPU acceleration unless a backend has been qualified.

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
The package does not bind or claim Reference local runner's port; Marina retains its documented
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


### Local milestone packaging and qualification

A local archive is a preview until the gates in WP-0111 pass. Build both client
and daemon from the same clean commit with the pinned native payload, then run:

```sh
scripts/package-talvik-unix.sh target/release "$HOME/.marina/lib" /tmp/talvik-local-preview.tar.gz
TMPDIR=/tmp scripts/smoke-release-install.sh /tmp/talvik-local-preview.tar.gz
python3 scripts/test-talvik-installers.py
```

The archive contains binaries, native libraries, installers, build identity, and
license; it does not include models, user configuration, or authentication tokens.
The smoke check installs into an isolated directory with developer library paths
removed, starts a daemon without model downloads, checks CLI status, stops it,
and verifies uninstall preserves model state.

For opt-in real-model API qualification, start a native daemon and register the
Qwen Coder and vision models, then run `python3 scripts/test-talvik-native-api.py`.
Set `TALVIK_TEST_TOKEN_FILE` to a private token file; optional `TALVIK_TEST_BASE_URL`,
`TALVIK_TEST_VISION_MODEL`, `TALVIK_TEST_CODER_MODEL`, and `TALVIK_TEST_REPORT`
select the service, model IDs, and JSON evidence path. This is a bounded check
of chat, coding, vision, standard SSE and a tool-result round trip. It does not
execute filesystem tools or certify any IDE's Agent/Plan mode.
