# Open Intelligence Desktop (OID)

OID is a Linux-first AI desktop/OS. The long-term product lets a user prompt
the computer to inspect its state, make a plan, operate through bounded skills,
verify the result, and recover safely when something fails.

The product has three distinct layers:

- **OID** — the Linux AI desktop/OS, policy, approvals, skills, verification,
  recovery, and evidence plane.
- **Talvik** — the provisional name of the standalone model runtime and local
  AI service. Talvik is the cross-platform release for Linux, macOS, and
  Windows.
- **OID OS Model** — a future specialized model for computer-management plans
  and typed tool proposals, served by Talvik but never granted direct OS
  authority.

The Rust CA-Clipper subsystem is a separate Linux OID product track. It is not
part of Talvik and is being migrated from the legacy archive only after
inventory, provenance, licensing, and data-compatibility review.

## Status

This repository contains the Rust workspace, Talvik runner, OID runtime, and
interactive console reference client. The local runner currently supports a
backend-neutral llama.cpp boundary, GGUF discovery, model lifecycle, tokenizer
routing, streamed text generation, cancellation, metrics, local bearer tokens,
and a small OpenAI-compatible IDE bridge.

The current runner is a focused CPU-first release slice: one loaded model, one
active generation, and local-first operation. Optional ordered local-model
fallback is available but disabled by default. Chat sessions, tools,
embeddings, full provider secrets, capability-aware remote routing, production
scheduling, and Linux distro packaging remain later work.

The local Talvik installation retains the compatible `$HOME/.marina` home;
native libraries use `$HOME/.marina/lib` and models use `$HOME/.marina/models`:

```sh
export LLAMA_CPP_LIB_DIR="${LLAMA_CPP_LIB_DIR:-$HOME/.marina/lib}"
```

Talvik keeps persistent user configuration in `$HOME/.marina/config.yaml`
(`%USERPROFILE%\.marina\config.yaml` on Windows):

```yaml
name: Talvik
model_directory: ~/.marina/models
registry_path: ~/.marina/models.registry
state_directory: ~/.marina
```

Environment variables such as `TALVIK_HOME`, `TALVIK_CONFIG`,
`TALVIK_MODEL_DIR`, `TALVIK_REGISTRY`, and `TALVIK_STATE_DIR` override the YAML
file. The equivalent `MARINA_*` variables remain compatibility aliases. See
[Talvik Model Runtime](docs/TALVIK.md)
for commands, routing configuration, security defaults, and migration rules.

The directory must contain the native `libllama` library. If it is not set or
does not contain a usable library, the backend reports unavailable.

## IDE access

Create a local Talvik access token:

```bash
TOKEN="$($HOME/.marina/bin/talvik auth token create ide 2>/dev/null)"
```

Start Talvik and the dependency-free IDE bridge:

```bash
$HOME/.marina/bin/talvikd
python3 scripts/marina-openai-proxy.py
```

Configure an OpenAI-compatible IDE with:

```text
Base URL: http://127.0.0.1:11435/v1
API key:  the printed Talvik token
```

The bridge supports `/v1/models`, `/v1/chat/completions`,
`/v1/completions`, bearer authentication, multi-turn text messages, and SSE
stream responses. The token authorizes access to Talvik; it is not a remote
provider API key. Remote BYOK provider credentials are not implemented yet.

Talvik also has a native HTTP API. Enable it explicitly on loopback with
`TALVIK_HTTP_ADDR=127.0.0.1:11434 talvikd` on Linux/macOS; Windows uses
`127.0.0.1:11435` because its local IPC transport uses `127.0.0.1:11434`.
The HTTP API uses the same runtime service as the local transport and requires
a bearer credential by default. The Python bridge is kept for migration and
compatibility testing, not as the production service boundary.

## GitHub releases

Tagging a release or manually starting [the Talvik release workflow](.github/workflows/talvik-release.yml)
builds native CPU artifacts on their target machines:

- Linux x86_64.
- macOS Intel.
- macOS Apple Silicon.
- Windows x86_64.

Each draft release contains `talvikd`, `talvik`, the matching pinned
llama.cpp libraries, license metadata, and SHA-256 checksums. Models are never
bundled into releases. Windows and native llama.cpp smoke tests must run on
their respective GitHub runners; this Mac workspace cannot validate them. A
friend's Apple Silicon Mac can also build and smoke-test the
`aarch64-apple-darwin` archive locally when we need hardware-specific
verification or a fallback upload path.

### Windows user installation

On Windows, extract the release ZIP and run PowerShell as the normal user:

```powershell
.\install-model-runner.ps1 -ArchivePath .\talvik-windows-x86_64.zip
```

The installer uses `%USERPROFILE%\.marina` by default:

```text
%USERPROFILE%\.marina\bin       talvikd.exe and talvik.exe
%USERPROFILE%\.marina\lib       llama.dll and ggml*.dll
%USERPROFILE%\.marina\models    local GGUF models
%USERPROFILE%\.marina\state     registry and runtime state
%USERPROFILE%\.marina\logs      user-scoped logs
```

It does not require administrator privileges or install system-wide files.
Set `TALVIK_HOME` to override this complete Talvik directory.

### Windows source build

For a source build, open a Visual Studio Developer PowerShell with the C++
workload and Windows SDK available, then run:

```powershell
.\scripts\setup-llama-windows.ps1
$env:LLAMA_CPP_LIB_DIR = "$env:USERPROFILE\.marina\lib"
$env:LLAMA_CPP_REQUIRED = "1"
cargo build --release -p oid-console --bin talvik --bin talvikd
```

Place a GGUF model under `%USERPROFILE%\.marina\models`, then create a Talvik
token and prepare the native DLL search path:

```powershell
$apiKey = (& .\target\release\talvik.exe auth token create windows-smoke).Trim()
$env:MARINA_API_KEY = $apiKey
$env:MARINA_TCP_ADDR = "127.0.0.1:11434"
$env:MARINA_HTTP_ADDR = "127.0.0.1:11435"
$env:Path = "$env:USERPROFILE\.marina\lib;$env:Path"
```

Start `target\release\talvikd.exe` in that environment. In a second PowerShell
window, run the basic HTTP smoke test:

```powershell
$env:MARINA_MODEL = "your-model-id"
.\scripts\smoke-marina-windows.ps1
```

The setup script builds the pinned llama.cpp revision and produces the native
`.lib` and `.dll` files. The smoke test requires a running daemon and does not
replace the full native qualification in WP-0079. The API key is sent as a
Bearer credential to Marina's local HTTP API.

## Local model bring-up on macOS Intel or Apple Silicon

Build and install the pinned native dependency into `$HOME/.marina/lib` with:

```bash
./scripts/setup-llama-macos.sh
```

The separate model-runner installer creates the canonical model store at
`$HOME/.marina/models`. Import a model into that store with the runner client:

```bash
./scripts/install-model-runner.sh
export PATH="$HOME/.marina/bin:$PATH"
talvik import \
  https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf \
  qwen2.5-0.5b-instruct-q4_k_m
```

Then run OID from the repository root with:

```bash
LLAMA_CPP_LIB_DIR="$HOME/.marina/lib" \
DYLD_LIBRARY_PATH="$HOME/.marina/lib" \
cargo run -p oid-console --bin oid-console
```

## Talvik service and client

The model runner can be installed without the interactive console:

```bash
./scripts/install-model-runner.sh
export PATH="$HOME/.marina/bin:$PATH"
talvikd
talvik status
```

For repository-local development, build the release binaries and link them to
`./talvik/bin`:

```bash
cargo build --release -p oid-console --bin talvik --bin talvikd
./scripts/link-model-runner.sh
./talvik/bin/talvikd
./talvik/bin/talvik status
```

Use `--prefix "$HOME/.local"` only when you intentionally want the
executables in `$HOME/.local/bin`; the canonical Talvik installation is
`$HOME/.marina`. Release archives include the matching native libraries under
`$HOME/.marina/lib` and the installer places them there. A source/local build
can use `./scripts/install-model-runner.sh --native-lib-dir DIR`; without that
payload the runner is control-plane-only and reports llama.cpp unavailable.
`talvik import` manages `$HOME/.marina/models`, while
`setup-llama-macos.sh` is the local development helper that builds the native
payload into `$HOME/.marina/lib`.

The persistent local runtime can be started independently of clients:

```bash
cargo run -p oid-console --bin talvikd
cargo run -p oid-console --bin talvik -- status
cargo run -p oid-console --bin talvik -- models
```

Talvik owns the `Runtime::start` composition root and keeps model state in the
daemon process. `talvik` communicates over the local transport: Unix socket
at `$HOME/.marina/marina.sock` on Linux/macOS, or loopback TCP on Windows via
`MARINA_TCP_ADDR`. Closing the client does not stop Talvik or unload its model.
The native transport remains local-only; the IDE HTTP bridge is a separate
loopback compatibility edge.

At the OID prompt, use `:model load qwen2.5-0.5b-instruct-q4_k_m`, followed by
`:generate The capital of France is`. Use `:generate --max-tokens 4 ...` for a
short smoke test; the default is 128 tokens. Ctrl+C during generation cancels
the active request and leaves the console available for another command.

The standalone runner can import a local or remote GGUF artifact:

```bash
talvik import /path/to/model.gguf
talvik import https://host.example/model.gguf model-id SHA256_HEX
talvik models
talvik load model-id
talvik run model-id "Hello from Talvik"
```

At startup Talvik also discovers existing `.gguf` files recursively in the
compatible Marina store, `~/Models`, Hugging Face/Python caches (`HF_HOME`,
`TRANSFORMERS_CACHE`, `XDG_CACHE_HOME`), ModelScope caches, and common Ollama
cache roots. Discovery registers files in place; `model pull` is the explicit
copy/download path into `$HOME/.marina/models`.

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
| `oid-runtime` | Marina production runtime lifecycle, service interfaces, and backend composition |
| `oid-llama-cpp-adapter` | Public package boundary for the native llama.cpp adapter |
| `oid-llama-cpp-sys` | Direct, opt-in official llama.cpp C API link boundary |
| `oid-console` | Keyboard-first interactive Marina console reference client |
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
cargo clippy --workspace --all-targets --all-features
```

Strict Clippy is not yet clean because the existing handwritten SHA-256 and
documentation surfaces still emit warnings. It is a release hardening task,
not evidence that model execution is unavailable.

The project targets Linux, Wayland first, and the latest stable Rust toolchain. Platform integration will be introduced behind traits and adapter crates as the design matures.

## License

OID is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
