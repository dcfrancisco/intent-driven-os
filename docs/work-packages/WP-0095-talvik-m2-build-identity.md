# WP-0095: Talvik M2 Build and Runtime Identity

Status: Implemented; deterministic and binary smoke verification passed.

## Scope

Expose independently identifiable CLI, daemon, IPC, and HTTP build metadata
without changing scheduling or native inference behavior.

## Implementation

- `console/build.rs`
- `console/src/build_info.rs`
- Talvik CLI, daemon, IPC status, and HTTP `/version`/`/v1/version`

## Acceptance criteria

- `talvik --version` and `talvikd --version` expose package version.
- Verbose and JSON forms expose Git SHA, dirty flag, target, and profile.
- HTTP exposes machine-readable daemon identity.
- IPC status exposes daemon identity and version mismatch.
- Unavailable native revision remains unknown/null, never fabricated.

## Verification

`cargo fmt --all`, Talvik binary tests, strict console Clippy, and executable
version commands passed on macOS x86_64. Live daemon mismatch and HTTP endpoint
execution remain native qualification work.
