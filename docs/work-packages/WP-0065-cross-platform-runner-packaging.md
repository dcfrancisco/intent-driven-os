# WP-0065: Cross-Platform Runner Packaging

## Purpose

Package the standalone Marina runner for Linux, macOS, and Windows without
requiring the OID interactive console.

## Scope

- User-installable binaries for `marina` and `marinactl`.
- Optional system package/install paths.
- OS-aware Marina home, binary, native-library, model, socket, and registry
  locations.
- Upgrade, uninstall, version, checksum, and architecture/accelerator
  selection behavior.
- Platform-matched native backend packaging (`libllama`, `libggml`, and
  Windows DLLs) inside each runner package.

## Acceptance criteria

- Fresh Linux, macOS, and Windows environments can install and run `marina`
  plus `marinactl status` without `oid-console`.
- Reinstall upgrades binaries without deleting models or runtime state.
- Installer selects the correct CPU/GPU variant or reports an explicit
  unsupported-machine result.
- Every release archive contains `lib/` with the native backend payload, and
  the packaged binary starts without global library-path configuration.
- No model is downloaded into the source repository.
- CI validates package contents and a clean uninstall removes only installed
  runner artifacts.

## Dependencies

WP-0064; ADR-0012; ADR-0013.

## Implementation status (2026-10-07)

The release workflow now builds the platform-native llama.cpp payload into
each archive's `lib/` directory, validates that `libllama`/`llama.dll` exists,
and includes the user installer. Marina binaries receive a relocatable Unix
library path; Windows installers mirror DLLs beside the executables. Actual
host smoke verification remains a release-runner responsibility for Windows
and both macOS architectures.
