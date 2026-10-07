# WP-0064: Individual Model Runner Installer

## Purpose

Make the model runner independently installable and operable without the OID
interactive console.

## Scope

Release artifacts for `marina` and `marinactl`, prefix selection, user-scoped
state/model directories, installation of the platform-matched native
`libllama`/`libggml` payload into `lib/`, native-backend availability
messaging, version reporting, upgrade/uninstall behavior, and later
`.deb`/`.rpm` packaging.

## Acceptance criteria

- A clean machine can install the runner with `scripts/install-model-runner.sh`
  and run `marina` plus `marinactl status` without `oid-console`.
- Installation does not download models.
- A release archive installs its included native backend without rebuilding the
  console; a source/local install may explicitly provide `--native-lib-dir`.
- The installed binary resolves the adjacent user-scoped `lib/` payload.
- Re-running the installer upgrades binaries without deleting models or state.
- A documented uninstall removes only installed binaries and service metadata,
  never user models unless explicitly requested.
- Linux CI validates the artifact contents and no-console dependency at runtime.

## Dependencies

WP-RUNTIME-002, WP-0059, WP-0062; ADR-0010, ADR-0011, ADR-0012.

## Implementation status (2026-10-07)

The Unix installer accepts `--native-lib-dir` and installs the native payload
under the same user prefix as the binaries. The Windows installer requires
`lib/` in the release archive and prepares the DLL loader layout. Package
upgrade/uninstall lifecycle and distro-native packages remain open.
