# ADR-0012: Individually Installable Model Runner

- Status: Accepted
- Date: 2026-10-01
- Related: ADR-0004, ADR-0006, ADR-0010, ADR-0011; WP-0064

## Decision

The model runner is a separately installable product surface. Users may install
the `marina` runtime service and `marinactl` client without installing or
starting the interactive OID console. The interactive console remains an
optional client of the same local runtime.

The first installer is `scripts/install-model-runner.sh`. It installs only the
two runner binaries into a selectable prefix (default `$HOME/.local`), creates
the per-user Marina state/model directory, and does not require a native
llama.cpp library at installation time. The runtime reports the backend as
unavailable until `LLAMA_CPP_LIB_DIR` points to a compatible library.

The distribution boundary is:

```text
model-runner install -> marina daemon -> local API/socket -> marinactl or oid-console
                                      `-> optional llama.cpp adapter + GGUF model
```

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
- Packaging must eventually provide upgrade, uninstall, checksums, and Linux
  package formats; the shell installer is the initial development path.
