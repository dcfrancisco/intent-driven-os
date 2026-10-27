# Talvik Distribution and Bootstrap Evidence

Date: 2026-10-28

This record covers the implementation slice for standalone distribution, default-model bootstrap, and startup readiness. It is evidence for the current source revision, not a platform release qualification.

## Implemented

- Default Qwen2.5-0.5B-Instruct Q4_K_M provisioning is configured from an immutable upstream artifact revision with a pinned SHA-256. The package manifest is informational; package installation never downloads a model.
- `TALVIK_AUTO_BOOTSTRAP`, `TALVIK_AUTO_DOWNLOAD`, and `TALVIK_AUTO_LOAD` are supported, with `TALVIK_*` taking precedence over the legacy `MARINA_*` compatibility names.
- The daemon binds its IPC and HTTP listeners before provisioning. `/healthz` reports service availability and bootstrap state; `/readyz` remains non-ready until a model has loaded successfully.
- Bootstrap failures leave the daemon available for diagnostics and do not claim inference readiness.
- Standalone DEB and RPM builder scripts install `talvik`, `talvikd`, native libraries, the user service unit, and the default-model manifest without removing `.marina` or downloading models during installation.

## Verification executed

| Check | Result | Evidence |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | Completed on the changed workspace. |
| `CARGO_TARGET_DIR=/tmp/talvik-bootstrap-target cargo check -p oid-runtime -p oid-console` | PASS | Runtime and console compile with the native boundary configured. |
| `CARGO_TARGET_DIR=/tmp/talvik-bootstrap-target cargo clippy -p oid-runtime -p oid-console --all-targets --all-features -- -D warnings` | PASS | No findings in the changed runtime/console targets. |
| `CARGO_TARGET_DIR=/tmp/talvik-bootstrap-target cargo test --workspace` | PASS with environment-limited process tests | Workspace unit tests completed successfully; process-signal tests attempted to bind local sockets and reported macOS sandbox `Operation not permitted`/address-in-use conditions. |
| `bash -n scripts/package-talvik-deb.sh scripts/package-talvik-rpm.sh` | PASS | Both packaging scripts parse successfully. |
| Package build on this host | NOT EXECUTED | `dpkg-deb` and `rpmbuild` are not available on this macOS host. |

## Not yet qualified

- Native GGUF inference against freshly built release artifacts was not rerun in this slice; existing user-managed `.marina` binaries and models were not stopped or modified.
- DEB/RPM installation, Linux user-service lifecycle, Windows installation, Docker, macOS arm64, and Linux/Windows native qualification require their respective hosts or build tools.
- Bootstrap coordination is single-flight within one daemon process. A process-safe cross-daemon lock and power-loss/registry-journal qualification remain release work.
- No package release or platform GO decision is implied by these source-level checks.

## Reproduction commands

```bash
cargo fmt --all -- --check
CARGO_TARGET_DIR=/tmp/talvik-bootstrap-target cargo check -p oid-runtime -p oid-console
CARGO_TARGET_DIR=/tmp/talvik-bootstrap-target cargo clippy -p oid-runtime -p oid-console --all-targets --all-features -- -D warnings
CARGO_TARGET_DIR=/tmp/talvik-bootstrap-target cargo test --workspace
bash -n scripts/package-talvik-deb.sh scripts/package-talvik-rpm.sh
```

For a host with packaging tools and freshly built binaries:

```bash
scripts/package-talvik-deb.sh --version 0.1.0 --talvikd target/release/talvikd --talvik target/release/talvik --native-lib-dir /path/to/native/lib --output artifacts
scripts/package-talvik-rpm.sh --version 0.1.0 --talvikd target/release/talvikd --talvik target/release/talvik --native-lib-dir /path/to/native/lib --output-dir artifacts
```

