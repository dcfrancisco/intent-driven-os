# Open Intelligence Desktop (OID)

Current Talvik implementation, test evidence and RC blockers are recorded in the [reports index](docs/reports/README.md). The [2026-11-01 status](docs/reports/TALVIK-STATUS-2026-11-01.md) distinguishes committed source from installed binaries and live IDE acceptance.

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

The current runner is a focused CPU-first release slice: multiple resident
models, one active generation, FIFO waiting, bounded LRU eviction under memory
pressure, streaming, cancellation, embeddings, and local-first operation.
Optional ordered local-model fallback is available but disabled by default.
Talvik does not edit project files, execute autonomous tools, or own OID/DW/OIP
policy. Concurrent generation, GPU scheduling, remote providers, and distro
packaging remain outside M1.

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
default_model: gemma-4-e4b-it-q4_0
```

Environment variables such as `TALVIK_HOME`, `TALVIK_CONFIG`,
`TALVIK_MODEL_DIR`, `TALVIK_REGISTRY`, `TALVIK_STATE_DIR`, and
`TALVIK_DEFAULT_MODEL` overrides the YAML file. On daemon startup, Talvik loads
the default model and downloads its pinned GGUF and vision-projector artifacts
if they are not installed (about 4.8 GiB for the Gemma default). Set
`default_model: none` to disable automatic loading. The equivalent
`MARINA_*` variables remain compatibility aliases. See
[Talvik Model Runtime](docs/TALVIK.md)
for commands, routing configuration, security defaults, and migration rules.
See the [Talvik compatibility roadmap](docs/TALVIK-COMPATIBILITY-ROADMAP.md)
for the declared OpenAI/Ollama targets, remaining build gates, and
differentiating runtime work.

The directory must contain the matching native llama.cpp libraries. Tool-template
execution also requires the private `oid-chat-template` library built by the
platform setup helpers. If the core native library is missing, the backend
reports unavailable; missing template/parser support must not be advertised as
working tools. See [ADR-0035](docs/adr/0035-native-embedded-chat-templates.md).

## IDE access

Use Talvik's native HTTP API for IDE access. It shares model state, scheduling
and authentication with the local client. Configure an OpenAI-compatible IDE
with:

```text
Base URL: http://127.0.0.1:11435/v1
Chat Completions URL, when requested: http://127.0.0.1:11435/v1/chat/completions
API key: a private local Talvik access token
```

Create a local token with `talvik auth token create ide` and store it through
the IDE's secret picker. Do not commit it. The token authorizes local Talvik
access; it is not a remote provider credential.

If Talvik is already managed by a service manager, use that instance and its
configured HTTP address. Do not start a second daemon or the Python proxy on
the same port. For a new foreground setup with no running instance:

```bash
TALVIK_HTTP_ADDR=127.0.0.1:11435 "$HOME/.marina/bin/talvikd"
```

The native API includes `/v1/models`, Chat Completions, other declared inference
routes, bearer authentication and SSE streaming. See the exact supported
subset in [OpenAI compatibility](docs/TALVIK-OPENAI-COMPATIBILITY.md); endpoint
availability is not full compatibility or Agent qualification. The Python
bridge is retained for migration/testing and is not required for native HTTP.

Use the [VS Code presets](docs/examples/vscode/README.md) for the local Coding
and read-only Plan configurations. A tool-capable model returns calls; VS Code
executes them under its own approval flow and sends results back. Talvik does
not execute IDE commands. The [current report](docs/reports/TALVIK-STATUS-2026-11-01.md)
records the pending full IDE acceptance and latency gates.

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

#### Manually uploading a Windows preview

GitHub CLI authentication is interactive and does not require putting a token
in this repository or in the release command. Sign in through the browser and
confirm the active account:

```powershell
gh auth login --hostname github.com --git-protocol https --web
gh auth status
```

From the repository root, set a new preview tag and the commit that produced
the ZIP, then upload the archive and checksum as a public prerelease. Replace
the example tag and commit before running; the tag must not already exist.

```powershell
$tag = "windows-preview-<short-commit>"
$sourceCommit = "<full-commit-sha-that-built-the-zip>"
gh release create $tag `
  "target\release\talvik-windows-x86_64-preview.zip" `
  "target\release\talvik-windows-x86_64-preview.zip.sha256" `
  --repo "dcfrancisco/intent-driven-os" `
  --target $sourceCommit `
  --title "Talvik Windows x64 preview" `
  --notes "Unsigned Windows x64 preview. Windows runtime qualification is incomplete; see README-WINDOWS.txt in the ZIP." `
  --prerelease
```

`--prerelease` publishes the release so testers can download it. Do not put
personal access tokens or `gh` credential data in source files or command
arguments; see the [GitHub CLI authentication guide](https://cli.github.com/manual/gh_auth_login).

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

The ZIP installer does not register or start `talvikd` as a Windows service.
Start it in a normal PowerShell window:

```powershell
$talvik = Join-Path $env:USERPROFILE ".marina\bin"
& (Join-Path $talvik "talvikd.exe")
```

In another window, check the daemon:

```powershell
$env:Path = "$env:USERPROFILE\.marina\bin;$env:Path"
talvik status
talvik models
```

Stop the foreground daemon with Ctrl+C. A conventional Windows service
requires an elevated installation; for non-admin use, a per-user logon task
could be considered later, but it is not currently installed or supported.
A per-user MSI is also only proposed and is explicitly not a service installer;
see [ADR-0030](docs/adr/0030-talvik-per-user-windows-msi.md) and
[WP-0107](docs/work-packages/WP-0107-talvik-per-user-msi-installer.md).

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

Start `target\release\talvikd.exe` in that environment. The OID Console is a
Linux-first MarinaOS component and is not part of the Windows Talvik release.
Use Talvik's HTTP/OpenAI-compatible API from external clients; the following
smoke test validates that service boundary.

In another PowerShell window, run the basic HTTP smoke test:

```powershell
$env:MARINA_MODEL = "your-model-id"
.\scripts\smoke-marina-windows.ps1
```

The setup script builds the pinned llama.cpp revision and produces the native
`.lib` and `.dll` files. The smoke test requires a running daemon and does not
replace the full native qualification in WP-0079. The API key is sent as a
Bearer credential to Marina's local HTTP API.

## Talvik smoke test on Apple Silicon (M4)

This is a native macOS smoke test for Talvik, not a claim that the M4 release
gate is already qualified. Use a native arm64 terminal (not Rosetta), Rust,
Xcode Command Line Tools, and CMake (install it with `brew install cmake` if
needed). Confirm the machine and OS first:

```bash
uname -m
sw_vers
```

`uname -m` should report `arm64`. Build the pinned llama.cpp dependency and
install Talvik under the standard per-user prefix. On Apple Silicon the setup
script enables the Metal build option by default:

```bash
./scripts/setup-llama-macos.sh
export LLAMA_CPP_LIB_DIR="$HOME/.marina/lib"
export LLAMA_CPP_REQUIRED=1
export DYLD_LIBRARY_PATH="$LLAMA_CPP_LIB_DIR${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
./scripts/install-model-runner.sh --native-lib-dir "$LLAMA_CPP_LIB_DIR"
export PATH="$HOME/.marina/bin:$PATH"
talvik version
```

Start Talvik in Terminal 1 with HTTP bound to loopback:

```bash
TALVIK_HTTP_ADDR=127.0.0.1:11434 \
talvikd
```

The configured Gemma 4 E4B-it default and vision projector download if missing
and load during startup. The first download is about 4.8 GiB; wait for the
daemon to report that the model loaded.
In Terminal 2, check the daemon and run a local generation through Talvik's
IPC client:

```bash
talvik status
talvik models
talvik run --default "What is 36 x 36? Answer with one number."
talvik --json status
```

Optional: exercise the external-client HTTP boundary on the same loopback-only
daemon:

```bash
TOKEN="$(talvik auth token create m4-smoke 2>/dev/null)"
curl --fail-with-body --silent --show-error \
  http://127.0.0.1:11434/v1/chat/completions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"gemma-4-e4b-it-q4_0","messages":[{"role":"user","content":"Reply exactly: Talvik on M4 works"}],"max_tokens":24}'
unset TOKEN
```

Stop the daemon with Ctrl+C in Terminal 1. Send back `uname -a`, `talvik
version`, the JSON status, the CLI/API results, and any daemon errors; do not
send the bearer token. Record the reported execution placement rather than
assuming Metal was used: a successful smoke test does not by itself qualify
Metal acceleration or replace the full native lifecycle gate in
[WP-0080](docs/work-packages/WP-0080-marina-macos-arm64-qualification.md).

The OID Console is a Linux-first MarinaOS component. This macOS procedure
tests Talvik directly and does not require or launch `oid-console`.

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

At the OID prompt, use `:model load gemma-4-e4b-it-q4_0`, followed by
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
`TRANSFORMERS_CACHE`, `XDG_CACHE_HOME`), ModelScope caches, and common Reference local runner
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
