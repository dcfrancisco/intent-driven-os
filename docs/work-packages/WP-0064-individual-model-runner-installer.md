# WP-0064: Individual Model Runner Installer

## Purpose

Make the model runner independently installable and operable without the OID
interactive console.

## Scope

Release artifacts for `marina` and `marinactl`, prefix selection, user-scoped
state/model directories, native-backend availability messaging, version
reporting, upgrade/uninstall behavior, and later `.deb`/`.rpm` packaging.

## Acceptance criteria

- A clean machine can install the runner with `scripts/install-model-runner.sh`
  and run `marina` plus `marinactl status` without `oid-console`.
- Installation does not download models or require `libllama`.
- A configured native backend can be used after installation without rebuilding
  the console.
- Re-running the installer upgrades binaries without deleting models or state.
- A documented uninstall removes only installed binaries and service metadata,
  never user models unless explicitly requested.
- Linux CI validates the artifact contents and no-console dependency at runtime.

## Dependencies

WP-RUNTIME-002, WP-0059, WP-0062; ADR-0010, ADR-0011, ADR-0012.
