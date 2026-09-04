# Open Intelligence Runtime and Console

This project is an open-source AI-native runtime and console for Linux. The initial system is intentionally focused on two pieces: an Intelligent Runtime that manages models as system resources, and an AI Console that provides a keyboard-first, retro terminal interface to that intelligence.

The runtime has no UI. It manages model lifecycle, hardware resources, backend adapters, streaming, security, logging, and health. The console is one client: its `int>` prompt expresses intent, while the runtime and future operation layer produce safe, auditable plans and results.

## Status

This repository contains the Phase 5 Rust workspace and interactive console
reference client. The runtime now has a backend-neutral native llama.cpp
integration boundary, GGUF discovery, model lifecycle management, tokenizer
routing, and streamed text generation with cancellation and metrics. The
native backend uses `LLAMA_CPP_LIB_DIR` at build time. For the local Marina
installation, set it to `/Users/dannyfrancisco/.marina/models`; the portable
default form is `$HOME/.marina/models`:

```sh
export LLAMA_CPP_LIB_DIR="${LLAMA_CPP_LIB_DIR:-$HOME/.marina/models}"
```

The directory must contain the native `libllama` library. If it is not set or
does not contain a usable library, the backend reports unavailable while the
console remains usable for architecture validation.

## Local model bring-up on macOS x86_64

Build the pinned, CPU-only native dependency with:

```bash
./scripts/setup-llama-macos.sh
```

The acceptance model used locally is `qwen2.5-0.5b-instruct-q4_k_m.gguf`.
Place it in `models/` (the directory is intentionally ignored by Git). For
the exact model used in the acceptance run:

```bash
curl -L --fail -o models/qwen2.5-0.5b-instruct-q4_k_m.gguf \
  https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf
```

Then run OID from the repository root with:

```bash
LLAMA_CPP_LIB_DIR=/tmp/oid-llama-cpu-build/bin \
DYLD_LIBRARY_PATH=/tmp/oid-llama-cpu-build/bin \
cargo run -p oid-console --bin oid-console
```

At the OID prompt, use `:model load qwen2.5-0.5b-instruct-q4_k_m`, followed by
`:generate The capital of France is`. Use `:generate --max-tokens 4 ...` for a
short smoke test; the default is 128 tokens. Ctrl+C during generation cancels
the active request and leaves the console available for another command.

Phase 5 adds independent `generate`, `complete`, and `explain` requests with
runtime-owned streaming, cancellation handles, generation events, and metrics.
There is no chat history, prompt template, tool execution, or agent loop.

The operation foundation now includes a read-only system-health flow, Linux
`/proc` inspection, append-only file evidence, and an explicitly approved
`create directory <path> --approve` operation with empty-directory rollback.
Milestone 3 makes the execution model canonical: every governed skill produces
an inspectable `OperationPlan` before approval, execution, verification, and
rollback.
Milestone 4 adds capability governance and routing: loaded capabilities can
register commands without shadowing native CLIs, and terminal input is
classified as native, dynamic, or intent mode with help and completion metadata.
It also adds durable approval lifecycle records with recovery inspection and
read-only directory and file inspection skills.
Milestone 5 adds `oid-operation-coordinator`, which composes those boundaries
into a durable end-to-end lifecycle with native and dynamic execution adapters.
Milestone 6 adds read-only process, filesystem, and systemd inspection skills,
plus an isolated D-Bus transport contract for future desktop adapters.

Milestone 7 corrects the console interaction model: OID Console remains a real
Linux shell, while OID-specific controls use an explicit `:` prefix. See
[WP-0055](docs/work-packages/WP-0055-terminal-identity-and-shell-passthrough.md).

## Workspace

| Crate | Responsibility |
| --- | --- |
| `oid-shared` | Runtime/console types, errors, configuration, and events |
| `oid-runtime` | Intelligent Runtime lifecycle, service interfaces, and mock runtime |
| `oid-llama-cpp-adapter` | Public package boundary for the native llama.cpp adapter |
| `oid-llama-cpp-sys` | Direct, opt-in official llama.cpp C API link boundary |
| `oid-console` | Keyboard-first interactive AI Console reference client |
| `oid-common` | Shared models, errors, configuration, logging, and utilities |
| `oid-intent-runtime` | Intent contracts and lifecycle state machine |
| `oid-linux-skills` | Typed Linux operations, `/proc` health inspection, and approved directory skill |
| `oid-policy-engine` | Permission, authorization, and approval framework |
| `oid-verification-engine` | Post-operation validation, health checks, and rollback verification |
| `oid-evidence-engine` | In-memory and durable append-only evidence records and operation history |
| `oid-plugin-sdk` | Plugin traits, registration, and discovery contracts |
| `oid-model-runner` | Backend-neutral model loading, tokenization, and generation contract |
| `oid-desktop-shell` | Terminal, Wayland, D-Bus, systemd, and event integration boundaries |
| `oid-docs` | Documentation anchor crate |

See [ARCHITECTURE.md](ARCHITECTURE.md) for boundaries and dependency direction, [ROADMAP.md](ROADMAP.md) for sequencing, and [docs/adr/README.md](docs/adr/README.md) for the architecture decision record sequence.

See [docs/adr/README.md](docs/adr/README.md) for the architecture decisions and [docs/work-packages/README.md](docs/work-packages/README.md) for the initial work package plan.

Linux validation is documented in [docs/validation/linux.md](docs/validation/linux.md).
Run `./scripts/validate-linux.sh` for the reproducible Ubuntu Docker checks;
Docker results do not replace real Linux host validation.

## Build and test

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The project targets Linux, Wayland first, and the latest stable Rust toolchain. Platform integration will be introduced behind traits and adapter crates as the design matures.

## License

OID is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
