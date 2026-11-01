# Talvik manual qualification runbook

This is a continuation checklist, not a record of completed acceptance. See the [status report](TALVIK-STATUS-2026-11-01.md) for captured results. Recurring tests have been cancelled; run these checks manually and sequentially, with no simultaneous IDE inference or other benchmark.

## 1. Record identities and prepare native dependencies

Record `git rev-parse HEAD`, `git status --short`, `talvik --json version`, running daemon identity, model SHA-256, native source pin, native library hashes and host/device details. The source commit and installed daemon must not be inferred from each other. For the reported Intel installation the API base is `http://127.0.0.1:11435/v1`; `/readyz` is a service check, not inference acceptance.

Use the platform setup helper to build the pinned llama.cpp and chat-template libraries. For an existing pinned checkout, the extra renderer can be built with:

```sh
LLAMA_CPP_ROOT=/path/to/pinned/llama.cpp \
LLAMA_CPP_INSTALL_DIR="$HOME/.marina/lib" \
bash scripts/setup-chat-template.sh
```

Requires CMake and the native C++ toolchain. `CMAKE_COMMAND` can select an explicit CMake executable. Keep the renderer and ggml libraries from matching native sources. On Windows use `scripts/setup-llama-windows.ps1`; qualify its DLL loading on Windows itself.

## 2. Check and build the source

On the captured macOS Intel environment, Rust 1.88.0 and the installed native library directory were used:

```sh
export LLAMA_CPP_LIB_DIR="$HOME/.marina/lib"
export LLAMA_CPP_REQUIRED=1
export LLAMA_CPP_MTMD_REQUIRED=1
export LLAMA_CPP_CHAT_TEMPLATE_REQUIRED=1
export DYLD_LIBRARY_PATH="$LLAMA_CPP_LIB_DIR"
cargo +1.88.0 fmt --all -- --check
cargo +1.88.0 test --workspace --locked
cargo +1.88.0 clippy -p oid-llama-cpp-sys -p oid-console --all-targets --locked -- -D warnings
cargo +1.88.0 build --release --locked -p oid-console \
  --bin talvikd --bin talvik --bin oid-console --bin oid-marina-chat
```

`DYLD_LIBRARY_PATH` is for developer qualification, not a substitute for packaged loader-path validation. Other platforms need their own loader setup. The explicit required flags prevent a missing native dependency from silently producing a control-plane-only qualification build.

Ignored real-model tests need their model variables and `--ignored --nocapture`. The captured fixtures are:

| Test filter | Required variables |
| --- | --- |
| `tool_system_layout_matches_the_loaded_template` | `MARINA_TEST_GGUF`, `TALVIK_TEST_TOOL_ORDER=client-first` for the tested Qwen3 artifact, `tools-first` for the tested Qwen3.5 artifact |
| `persistent_context_reuses_a_compatible_prompt_prefix` | `MARINA_TEST_GGUF` (tested attention model) |
| `recurrent_checkpoint_reuse_matches_cold_output_and_clears_on_cancel` | `TALVIK_TEST_RECURRENT_GGUF` |
| `vision_input_uses_image_content_and_recovers_after_cancellation` | `TALVIK_TEST_VISION_GGUF`, `TALVIK_TEST_VISION_PROJECTOR` |
| `loaded_profile_applies_tokenizer_metadata_without_duplicate_bos` | `TALVIK_TEST_PROFILE_GGUF` |

Run each with `cargo +1.88.0 test -p oid-llama-cpp-sys --locked FILTER -- --ignored --nocapture`. Supply existing local GGUF/projector paths; fixture names and explicit model expectations do not mean all GGUF families are supported.

## 3. Install without duplicate daemons

Check for active/pending requests first. Finish or explicitly cancel only the test request you own before replacing binaries. Back up the current binaries, atomically replace them with the clean release, and restart the existing service manager entry. On this Mac that entry is `com.talvik.talvikd`; do not launch a second foreground daemon alongside it. Preserve configuration, registry, models and private credentials. Verify the service is ready, the running build matches the release, and only one managed daemon owns the configured socket/HTTP port.

Package checks must verify `oid-chat-template` is included and resolves without checkout paths or developer loader overrides. Existing archives are historical until rebuilt, hashed and smoke-tested.

## 4. Exercise the installed API

The existing qualification scripts read the private local token without displaying it. Do not paste tokens or raw authorization headers into reports.

```sh
TALVIK_TEST_REPORT=/tmp/talvik-action-selection.json \
  python3 scripts/test-talvik-action-selection.py
TALVIK_TEST_CODER_MODEL=qwen3-1.7b-q4_k_m \
TALVIK_TEST_REPORT=/tmp/talvik-native-api.json \
  python3 scripts/test-talvik-native-api.py
```

The selection script defaults to the Qwen3.5 4B candidate; use `TALVIK_TEST_MODEL` only for an explicitly recorded alternative. Both scripts depend on the locally configured model IDs, endpoint and matching assets. Inspect failures rather than counting transport completion as a pass. These probes do not execute a project build; their tool-result continuation is synthetic.

Check ordinary chat, vision, automatic and required tools, well-formed complete arguments, streamed finish reasons and `[DONE]`, correlated result continuation, cancellation and a succeeding next request. Record exact model, build, settings and results. Use the broader recovery/SDK qualifiers listed in compatibility work packages before asserting the whole API profile is qualified.

## 5. Prove the real VS Code workflow

Follow the [preset setup notes](../examples/vscode/README.md). Start a fresh conversation in the known Maven workspace using **Talvik Coding**, and send exactly:

```text
build this project, don't check anything. just build
```

Require a terminal tool invocation, command equivalent to `mvn -DskipTests package` (or the existing wrapper), actual exit status, BUILD SUCCESS, and skipped tests. Inspect the tool transcript: no file inspections, edits, `clean`, `install`, or test goals. The model's final prose is not execution evidence. Client approval prompts remain in force.

Test Ask independently with a self-contained question. Test **Talvik Plan** with a read-only planning request; require actual read results for file claims and no edits or terminal commands. Preserve sanitized command/result evidence and elapsed time. If any model narrates instead of calling a tool, record failure and keep the RC gate open.

## 6. Measure and decide

Use the same GGUF digest, native CPU/device settings, context, token budget, sampling, thinking control and full structured IDE request for Talvik and Ollama. Record template-induced input-token differences. Run without competing inference; measure cold load, cold/warm prefill, TTFT, decode throughput, queue delay, total tool-loop time and peak memory. Test repeated and changed-user prefixes and utility-model switches separately.

Do not infer a cold latency win from a cache-hit probe. Do not qualify GPU inference from device discovery. Publish per-build results and remaining failures; promote an RC only after the named IDE, API, recovery, performance and platform/install gates pass.
