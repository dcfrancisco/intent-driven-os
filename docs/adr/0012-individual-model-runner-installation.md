# ADR-0012: Individually Installable Model Runner

- Status: Accepted
- Date: 2026-10-01
- Related: ADR-0004, ADR-0006, ADR-0010, ADR-0011, ADR-0014; WP-0064, WP-0069

## Decision

The model runner is a separately installable product surface. Users may install
the `marina` runtime service and `marinactl` client without installing or
starting the interactive OID console. The interactive console remains an
optional client of the same local runtime.

The first installer is `scripts/install-model-runner.sh`. It installs the two
runner binaries into `$HOME/.marina/bin` by default and creates the per-user
Marina state/model directory. A release payload includes the platform-matched
native llama.cpp/ggml libraries in `$HOME/.marina/lib`; the installer copies
that payload as part of the same installation. Source/local builds may provide
the same payload with `--native-lib-dir DIR` or `LLAMA_CPP_LIB_DIR`. A
binaries-only install is explicitly control-plane-only and must report the
backend unavailable; it is not a complete local-inference installation.

Windows uses `scripts/install-model-runner.ps1` and is user-scoped by default.
It installs the release ZIP under `%USERPROFILE%\.marina` without requiring
administrator privileges or writing to `Program Files`. The Windows layout is
`bin`, `lib`, `models`, `state`, and `logs` under that user-owned directory.

The canonical per-user layout is:

```text
$HOME/.marina/
├── bin/       # marina and marinactl
├── lib/       # packaged native backend libraries: libllama/libggml or DLLs
├── models/    # downloaded/imported GGUF artifacts
├── config.yaml # user-scoped Marina configuration
├── marina.sock
└── models.registry
```

Repository-local `models/` files are not part of the installation contract.
Development builds may use `./marina/bin` symlinks, but installed commands
must resolve from a system or user bin directory.

The distribution boundary is:

```text
model-runner install -> marina daemon -> local API/socket -> marinactl or oid-console
                                      `-> optional llama.cpp adapter + GGUF model
```

The binary is linked with a relocatable runtime search path to `../lib` on
Linux/macOS. Windows packages retain the canonical libraries in `lib` and
also place required DLLs beside `bin/marina.exe`, because that is the native
Windows loader search location. No global library-path mutation is required.

## Rationale

Installing the runner independently supports headless machines, services,
automation, future desktop clients, and users who do not want the terminal UI.
It also makes the runtime lifecycle independently upgradeable from the console.

## Constraints

- Installation must not download models or silently enable network access.
- The console must not be required to start the daemon.
- State, socket, model, and log paths must be user-scoped by default.
- Native backend availability is a runtime configuration concern, not an
  installer failure.
- Packaging must provide upgrade, uninstall, checksums, offline installation,
  service integration, and Linux package formats; the shell installer is only
  the initial development path.
- User installation and system installation are separate modes with separate
  ownership, paths, service identities, sockets, registries, locks, and model
  stores.
- The first Windows release is user-only. A future Windows service/system
  installation is a separate product decision and must not silently broaden
  permissions or move user models.
- Native backends, GPU/device access, and remote-provider credentials require
  explicit package/configuration policy; the installer must not grant broad
  root or network authority.
