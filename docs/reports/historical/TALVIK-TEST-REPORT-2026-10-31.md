> Historical snapshot imported on 2026-11-01. Build identities, pass claims, pending work, installed versions, and push status below describe their original run, not the latest source. See the [current status report](../TALVIK-STATUS-2026-11-01.md). Local-only archive/image references may require the original artifact directory.

# Talvik test and working-status report

Latest API stabilization results: [API candidate report](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/TALVIK-API-RC-REPORT-2026-10-31.md>). The historical sections below describe earlier builds.

Updated: 2026-10-31T14:44:59+08:00. Platform qualified here: **Intel macOS, CPU + Apple BLAS**.
Branch: `feature/qwen-coding-assistants`.

**Result: a working local runner milestone preview, with improved CLI presentation.
It is not yet an RC or a production-readiness certification.** Native chat,
coding, vision, standard streaming and a sample API tool loop pass. Actual
VS Code Ask works with high latency. Actual Agent/Plan acceptance, acceleration,
and release-wide qualification remain open.

## Latest follow-up: OID Console dot prompt

Installed source commit `383a063` adds the dBase-style `. ` prompt, restrained
header and a single pinned **state · model** footer. It removes the directory
prompt, synthetic cursor glyph, startup model list and automatic telemetry dump.
The footer releases the terminal for native commands and returns afterward.
`:status` retains explicit runtime details. The legacy `full` status setting now
uses the same compact footer; `hidden` still removes it.

Verification on local Intel macOS:

- Full workspace: **258 passed, 0 failed, 5 ignored**.
- Final console rerun after the last status refinement: **133 passed**.
- Installed console pseudo-terminal checks: **7 passed**: clean header/dot/footer,
  resize and long input, native shell handoff and Unicode, streamed generation,
  handled termination restoration, hidden status, and plain piped diagnostics/clear.
- Strict lint for `oid-console` and formatting: **passed**.
- Installed-binary checks used the installation's native library paths, with no
  developer library override. The Talvik daemon was not restarted.

These counts overlap; they are not additive. Preview rendering uses an actual
console session connected to the local daemon and a harmless `printf` command;
it is not a new native inference qualification. Existing Agent/Plan, performance,
full-workspace lint and RC blockers below are unchanged.

Start it with `~/.marina/bin/oid-console`.

[Terminal preview](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/oid-console-preview.png>) ·
[Qualification record](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/oid-console-qualification.json>) ·
[Installed terminal tests](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/oid-console-terminal-tests.log>) ·
[Workspace tests](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/oid-console-workspace-tests.log>)

## Current installation

| Item | Verified state |
|---|---|
| Daemon | One `talvikd` process; health `Healthy`; `/readyz` returns `inference_ready: true` |
| OpenAI base URL | `http://127.0.0.1:11435/v1`, using the existing private API token |
| CLI and one-shot chat | Installed clean client build `7d775f3` |
| Running daemon | Clean build `7c3a5ed`; kept running during the client-only update |
| Native backend | llama.cpp commit `18443257a30c884d5332abb8e7dc43c7ffe42fda`, CPU + Apple BLAS; eight inference threads |
| User installation | `~/.marina`; CLI/API share the registry and IPC socket |
| Recurring overnight tests | **Paused**, rechecked for this report |
| Source control | Changes committed locally; working tree clean. Last push attempt failed because GitHub HTTPS credentials were unavailable. |

Readiness confirms that the service is available; it does not qualify every IDE
mode or guarantee a short queue. The service processes one generation at a time.

## What is working

| Capability | Evidence and practical limit |
|---|---|
| Native text chat | Qwen3.5 returned the correct answer to the short test prompt. |
| Coding | Qwen2.5 Coder returned a valid Python `clamp` implementation. |
| Vision | Qwen3.5 identified a generated red image as “Red.” |
| OpenAI Chat Completions streaming | Live content/tool deltas, finish reasons, requested usage, and `[DONE]` passed. This is evidence for the tested behavior, not full OpenAI API parity. |
| API tool selection and continuation | Model emitted `read_file` with its required `path`; a correlated synthetic tool result produced the expected final answer. The test did not execute a filesystem tool. |
| Output cleanup | No trailing `<` or empty thinking wrapper appeared in the final five live API checks. |
| Daemon lifecycle | Duplicate HTTP startup fails before model allocation; active Unix sockets are preserved; stale sockets can restart. Regression tests pass. |
| Request scheduling | A 64-request synchronized regression passed; actual IDE main/utility requests queued successfully. This is not a sustained native load-test certification. |
| Cancellation | Live long-generation cancellation and subsequent generation passed during stabilization. The new CLI also cancelled its queued request before the first token. |
| VS Code Ask | Actual extension logged success for Qwen3.5; Qwen Coder utility request also succeeded. Main acceptance request took about **47 seconds**. |
| Standalone archive | Existing Intel macOS preview passed isolated install/start/status/stop/uninstall with developer library paths removed; model state was preserved. |

### Configured models

| Model | Verified / configured role |
|---|---|
| `qwen2.5-coder-3b-instruct-q4_k_m` | Coding and utility text requests passed; tools/vision are not advertised for this configuration. |
| `qwen3.5-4b-Q4_K_M` | Chat, vision, sample API tools and actual VS Code Ask passed. This does not qualify actual IDE Agent/Plan. |
| `deepseek-r1-1.5b` | Registered and loaded as a text model; not covered by the latest five-check Qwen qualification. |

## CLI presentation delivered

The shared renderer is wired into `talvik run`, `oid-marina-chat`, and OID console
generation. The first two clients are installed locally; the updated optional
OID console was built.

- Waiting animation and elapsed time until model tokens arrive.
- Thinking sections for emitted leading reasoning tags; empty sections hidden.
  Generating labels for answer output. Internal reasoning phases are not inferred
  when the model does not emit a corresponding marker.
- Cyan labels/code, muted thinking text, bold Markdown headings and emphasis.
  This is lightweight highlighting, not full Markdown layout or language syntax highlighting.
- Compact time/token/speed footer when metrics are available; separate completion,
  cancellation, output-limit and context-limit labels.
- Ctrl+C handling while waiting and streaming; temporary progress clears on handled
  exits. One-shot chat cancellation exits with code 130 without a redundant failure message.
- `--plain`, `TALVIK_PLAIN=1`, nonempty `NO_COLOR`, and `TERM=dumb` support.
  Redirected output stays raw; the internal `DONE` record is no longer appended.

The live display demonstration encountered an existing large IDE request. Its
queued request displayed waiting time and cancelled promptly. Full response
formatting was verified against fixture streams in real pseudo-terminals;
that demonstration is not being counted as a new native answer test.

## Test results

| Check | Result | Scope / evidence |
|---|---|---|
| Full workspace tests after CLI renderer work | **255 passed, 0 failed, 5 ignored** | [Full workspace log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-ui-workspace-tests.log>) |
| Final console tests after cancellation/context-label refinement | **130 passed, 0 failed** | [Final console log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-cli-console-tests.log>) |
| Installed-client terminal acceptance | **6 passed** | Color streaming, no-color, plain/dumb terminals, exact piped text, cancellation before first token, failure cleanup; [terminal log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-cli-terminal-tests.log>) |
| Strict lint on changed clients | **Passed** | `talvik`, `oid-marina-chat`, `oid-console`; [client Clippy log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-cli-client-clippy.log>) |
| Formatting | **Passed** | `cargo fmt --all -- --check` |
| Real native API acceptance | **5 passed** | Chat, coder, vision, required tool call, correlated tool result; [API evidence](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-final-api-report.json>) |
| Installer contract tests | **5 passed** | Source/archive layouts, path escaping, native vision payload, retained state, incomplete payload rejection and host-library isolation |
| Real archive lifecycle | **Passed** | [Archive smoke log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-archive-smoke.log>) |
| Full workspace strict Clippy | **Still blocked** | Last full attempt failed on existing daemon HTTP helper lints and test assertions. Client-only success does not replace this gate; [full lint log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-strict-clippy.log>) |

These counts overlap and must not be added together. Shared module tests also run
in several client binaries. The final console rerun covers the last small client
refinements; the 255-test workspace run preceded those refinements. Five ignored
tests require explicit native GGUF/projector fixtures or grammar qualification;
they are not being counted as passes. Separate live Qwen tests do not replace them.

## Performance observations

The controlled 769-token CPU prompt returned the same correct `PERF_OK` response
before and after enabling Apple BLAS. Prompt-processing time improved from
**33.9 seconds to 25.4 seconds: 1.33× faster**. This is one hardware/profile sample,
not a universal throughput claim.

The final live API checks took: chat 42.66s, coder 6.69s, vision 5.38s, tool call
17.43s, and tool-result answer 23.86s. These are end-to-end times and can include
queue/model work; they are not comparable pure inference benchmarks. Actual
VS Code Ask took 47.4s; the utility request took 65.6s including queue wait.

Real Agent prompts observed locally contain about **22K tokens**. Cold CPU prompt
processing remains the main usability problem. An Intel Iris GPU is present,
but the tested Vulkan configuration generated incorrect output and is **not a
qualified accelerated profile**.

## What is still pending before RC

1. **Actual VS Code Agent and Plan acceptance:** demonstrate workspace tool use,
   a correlated tool result, and a useful final answer in the real IDE. A sample
   API tool loop is insufficient evidence for this gate.
2. **Usable large-prompt latency and acceleration:** profile/cache/prefill work;
   qualify any GPU profile for correctness before enabling it for users.
3. **Release checks:** clear full strict lint, run the ignored native fixture tests,
   and qualify overflow/deadline, disconnect, memory pressure, crash/restart and
   continuous-client recovery behavior on the final build.
4. **Release packaging and platform coverage:** produce one consolidated clean
   artifact containing the latest CLI and daemon, then qualify clean-machine
   install/upgrade on every advertised OS/architecture. Current evidence is Intel macOS only.
5. **Compatibility and CLI usability:** finish the route/client compatibility
   inventory. Full Ollama/OpenAI parity is not established. Standalone `talvik run`
   remains one-shot with 128-output-token / 4096-context defaults; interactive
   conversation/history and richer generation controls are further product work.
6. **Publishing:** authenticate GitHub, push the committed feature branch, and run
   remote CI. No RC tag or public release was created.

## How to test the installed version

Basic service and coding check:

```sh
~/.marina/bin/talvik status
~/.marina/bin/talvik models
curl http://127.0.0.1:11435/readyz
~/.marina/bin/talvik run qwen2.5-coder-3b-instruct-q4_k_m 'Write a short Python add function.'
```

Use a regular terminal to see colors/progress. `NO_COLOR` suppresses colors;
`--plain` or piping produces raw text. Ctrl+C cancels your CLI request. Avoid
starting another heavy acceptance run while an IDE request is already waiting.

Bounded development checks; these are not recurring tests:

```sh
cd /Users/abigailbangasan/codes/intent-driven-os
python3 scripts/test-talvik-terminal.py "$HOME/.marina/bin"
python3 scripts/test-talvik-installers.py
TALVIK_TEST_REPORT=/tmp/talvik-api-report.json python3 scripts/test-talvik-native-api.py
```

The terminal/installer checks use isolated fixtures. The API check uses the
existing private token file without printing it and sends real inference requests.

## Artifacts

- [Existing Intel macOS milestone preview archive](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-m2.1-preview-macos-intel.tar.gz>) and [SHA-256 checksum](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-m2.1-preview-macos-intel.tar.gz.sha256>).
  **This archive is build `7c3a5ed` and does not contain the latest CLI display update.**
- [Milestone/API qualification record](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-milestone-qualification.json>).
- [CLI qualification record](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-cli-ui-qualification.json>).
- [Current report health snapshot](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-report-snapshot.json>).
- [CPU baseline](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/cpu-prompt-baseline.json>) and [Apple BLAS comparison](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/cpu-prompt-apple-blas.json>).

Source references: [terminal behavior](</Users/abigailbangasan/codes/intent-driven-os/runtime/README.md:166>) and
[RC acceptance gates](</Users/abigailbangasan/codes/intent-driven-os/docs/work-packages/WP-0111-talvik-working-runner-stabilization.md>).
