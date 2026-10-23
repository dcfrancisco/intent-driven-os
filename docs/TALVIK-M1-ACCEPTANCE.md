# Talvik M1 acceptance matrix

This is the current closeout record. Historical dated evidence remains in the
qualification documents and is not replaced by this summary.

## Product and runtime gates

| Gate | Result | Evidence |
|---|---|---|
| OpenAI-compatible models/chat/SSE/embeddings | PASS (recorded) | `docs/qualification/macos-x86_64-talvik-m1-2026-10-23.md`; `scripts/qualify-openai-client.py` and native wire checks |
| Finish reason stop vs length | PASS | HTTP unit tests and recorded `max_tokens=1` SSE evidence |
| Tool call/tool-result round trip | PASS (recorded) | Real Qwen evidence in macOS x86_64 qualification; unsupported templates are rejected by capability gate |
| Multi-model native residency | PASS on macOS x86_64 | Qwen and DeepSeek loaded concurrently, independently generated, DeepSeek unloaded, Qwen recovered |
| Bounded LRU recovery, pinning, reload, FIFO queue, queued cancellation | PASS (deterministic) | `cargo test -p oid-runtime service::tests -- --test-threads=1` |
| Admission boundary and fail-closed rejection | PASS (deterministic) | Exact-boundary and one-byte-below tests |
| Native DeepSeek-R1 pressure recovery | NOT EXECUTED | GGUF unavailable on this host; no threshold was relaxed |
| Continue/Cline UI integration | DEFERRED | Owner acceptance by explicit product decision; not an M1 release gate |

## Build and release gates

Commands executed on the macOS x86_64 workspace:

```sh
cargo fmt --all -- --check
cargo test -p oid-runtime service::tests -- --test-threads=1
LLAMA_CPP_LIB_DIR="$HOME/.marina/lib" LLAMA_CPP_REQUIRED=1 \
  cargo build --release --bin talvik --bin talvikd
DYLD_LIBRARY_PATH="$HOME/.marina/lib" target/release/talvik --help
DYLD_LIBRARY_PATH="$HOME/.marina/lib" target/release/talvikd --help
./scripts/install-model-runner.sh --help
bash -n scripts/install-model-runner.sh
bash -n scripts/smoke-release-install.sh
```

Results: PASS. The temporary-prefix uninstall preservation check also passed;
model and state sentinel files remained after binary/native-library removal.
The source installer build path itself was not re-run because Cargo attempted
to reach crates.io and this host has no DNS/network access. Release artifacts
from the rebuilt source:

```text
target/release/talvik  a06d7f167274a467c01a6e21da68963310743619a1c9a8dc60b4da21fca5b4f4
target/release/talvikd 3a2a621385ce1406f1ec22c774fdb1666641eaf3381e67921b6529143a830992
```

Strict workspace Clippy now PASSes with the exact release command:
`cargo clippy --workspace --all-targets --all-features -- -D warnings`.
Behavior-neutral lint fixes and narrowly scoped allowances for the SHA-256
implementation are included in the current closeout. Full workspace testing
completed with one environment failure: the HTTP-resume fixture cannot bind a
local socket under the restricted runner (`Operation not permitted`). All
console process-signal tests passed in this run.

## Distribution and platform matrix

| Target | Build | Install/startup/smoke | Native GGUF | Decision |
|---|---|---|---|---|
| macOS x86_64 | PASS | PASS (recorded) | PASS (recorded) | GO for tested local release |
| Linux x86_64 | CI procedure prepared | NOT EXECUTED in this workspace | Historical host evidence only; latest branch not re-run | NO-GO / preview |
| macOS arm64 | Procedure prepared | NOT EXECUTED | NOT EXECUTED | NO-GO / unverified |
| Windows x86_64 | Procedure and PowerShell installer prepared | NOT EXECUTED | NOT EXECUTED | NO-GO / unverified |
| Docker CPU image | Dockerfile and Compose prepared; `docker compose ... config` PASS | NOT EXECUTED; Docker daemon socket permission denied | NOT EXECUTED | NO-GO / blocked |

Required host re-test commands are in `docs/qualification/windows-x86_64-talvik-m1.md`
and `docs/qualification/macos-arm64-talvik-m1.md`. Do not promote an archive
from cross-compilation alone: run `scripts/smoke-release-install.sh`, daemon
health/model listing, and the native GGUF lifecycle suite on the target host.

## Current decision

Talvik M1 is **feature-complete for the verified CPU-first macOS x86_64 slice**.
It is **not fully qualified as a cross-platform release**. Linux latest-branch,
macOS arm64, Windows x86_64, and Docker execution remain unverified or blocked
and must retain preview/NO-GO status. Strict workspace lint is now passing.
