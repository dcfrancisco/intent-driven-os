# Open Intelligence Runtime and Console

This project is an open-source AI-native runtime and console for Linux. The initial system is intentionally focused on two pieces: an Intelligent Runtime that manages models as system resources, and an AI Console that provides a keyboard-first, retro terminal interface to that intelligence.

The runtime has no UI. It manages model lifecycle, hardware resources, backend adapters, streaming, security, logging, and health. The console is one client: its `int>` prompt expresses intent, while the runtime and future operation layer produce safe, auditable plans and results.

## Status

This repository contains the Phase 2 Rust workspace foundation and interactive console reference client. The runtime uses deterministic mock services; AI inference, llama.cpp, hardware probing, and Linux operations are not implemented.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `oid-shared` | Runtime/console types, errors, configuration, and events |
| `oid-runtime` | Intelligent Runtime lifecycle, service interfaces, and mock runtime |
| `oid-console` | Keyboard-first interactive AI Console reference client |
| `oid-common` | Shared models, errors, configuration, logging, and utilities |
| `oid-intent-runtime` | Intent contracts and lifecycle state machine |
| `oid-linux-skills` | Typed Linux operations and skill traits |
| `oid-policy-engine` | Permission, authorization, and approval framework |
| `oid-verification-engine` | Post-operation validation, health checks, and rollback verification |
| `oid-evidence-engine` | Evidence records, audit trail, and operation history |
| `oid-plugin-sdk` | Plugin traits, registration, and discovery contracts |
| `oid-model-runner` | Placeholder interfaces for future optimized model execution |
| `oid-desktop-shell` | Terminal, Wayland, D-Bus, systemd, and event integration boundaries |
| `oid-docs` | Documentation anchor crate |

See [ARCHITECTURE.md](ARCHITECTURE.md) for boundaries and dependency direction, [ROADMAP.md](ROADMAP.md) for sequencing, and [docs/adr/README.md](docs/adr/README.md) for the architecture decision record sequence.

See [docs/adr/README.md](docs/adr/README.md) for the architecture decisions and [docs/work-packages/README.md](docs/work-packages/README.md) for the initial work package plan.

## Build and test

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The project targets Linux, Wayland first, and the latest stable Rust toolchain. Platform integration will be introduced behind traits and adapter crates as the design matures.

## License

OID is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
