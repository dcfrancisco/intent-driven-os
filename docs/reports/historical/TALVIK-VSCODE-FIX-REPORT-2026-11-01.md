> Historical snapshot imported on 2026-11-01. Build identities, pass claims, pending work, installed versions, and push status below describe their original run, not the latest source. See the [current status report](../TALVIK-STATUS-2026-11-01.md). Local-only archive/image references may require the original artifact directory.

# Talvik VS Code repair and latency qualification — 2026-11-01

Installed build: `37d44a1c319a60166d62259b55842460eab4abfb`, clean release build from `feature/qwen-coding-assistants`. This is a local stabilization milestone. Cold Agent/Plan latency still prevents an RC claim.


## Follow-up: build execution failure

The user's subsequent `build this project` request in Talvik Coding completed after 329.747s with narration ending in “Building the project...”, but **no terminal tool invocation**. No build ran in that completed request. Terminal access was configured. The passing read-only Coding probe above does not qualify build execution or general Agent reliability. The newer build-only prompt was saved as incomplete; the daemon had zero active or pending requests when inspected, so that saved state alone does not establish running inference.

The Coding preset now explicitly requires action requests to invoke tools and forbids claiming execution from prose alone (`672c550`). This instruction change is installed, but an actual build round trip has not yet verified its effectiveness. Build execution remains an open acceptance failure.

## Follow-up: build action failed

The user's subsequent “build this project, don't check anything. just build” request in **Talvik Coding** failed its task. Saved session/log evidence shows a completed text response after 329.747s, with **zero terminal invocations**. The model described building but did not execute a build. No compiler error or terminal approval rejection was recorded for that request. This is an action-selection reliability failure; the passing read-only tool check does not establish build, edit or terminal reliability. The saved request used the older cached mode instructions; the subsequently updated installed preset explicitly requires invoking an execution tool for action requests. That instruction change requires its own live IDE qualification and is not a confirmed fix. A narrow API probe with the updated instructions did select the terminal tool in 10.272s, but chose `mvn clean install` despite an explicit no-tests/build-only scope. No command was executed by that probe. Tool selection alone is therefore insufficient to qualify this workflow. See [selection probe](../evidence/historical/talvik-build-tool-selection.json).

## What changed

- Chat Completions sends valid assistant-role SSE immediately after admission and keep-alive comments during CPU prefill. Errors use data-only OpenAI JSON frames. Native tools are validated before emission.
- Native device discovery selects the actual CPU, rather than an auxiliary BLAS device. This reduced graph splits from 71 to 1 for the tested Coder model.
- Assistant tool history and tool results follow the loaded template's supported protocol. The synthetic plural tool-call wrapper has been removed.
- Lazy JSON syntax constraints prevent malformed tool-call JSON on supported non-thinking XML/JSON profiles. They do not enforce every argument schema. Thinking-enabled requests retain existing validation without these constraints.
- Corrected a five-argument native repetition-penalty sampler binding that previously supplied four arguments. A real sampler fixture checks the penalty and its 64-token window.
- Attention models reuse the longest verified exact prefix, including changed and shorter requests. Recurrent models retain their separately qualified checkpoint strategy.
- Text KV allocations follow demand, with bounded room for the next tool result. Existing allocations are reused whenever the actual request fits. Multimodal retains its qualified full allocation.
- Fixed `talvik import` dropping its source argument. New models were imported using pinned revisions and SHA-256 verification.

## VS Code setup

The existing local API secret reference is preserved. The OpenAI base URL is `http://127.0.0.1:11435/v1`.

| Workflow | Model and configuration |
| --- | --- |
| Ask / simple chat | Qwen2.5 Coder 1.5B, tools and vision disabled |
| Talvik Coding | Qwen3 1.7B, thinking disabled, focused file/search/edit/terminal tool set |
| Talvik Plan | Qwen3 1.7B, thinking disabled, read/search tools only |
| Vision | Qwen3.5 4B with its matching projector |

Select **Talvik Coding** or **Talvik Plan** in the mode picker. The source presets are in `docs/examples/vscode/`; installed copies are in VS Code's user `prompts` directory. The built-in broad Agent tool inventory still produces a much larger cold prompt. A small model's declared tool capability does not establish reliable tool-result quality: Coder 1.5B failed that continuation check and is deliberately configured for chat only.

## Evidence

- Final installed-build workspace regression: **278 passed, 9 ignored native fixtures, zero failures**. Strict Clippy passed, including the final headroom adjustment.
- Explicit native fixtures passed for attention cold/warm/shorter-prefix reuse, context growth, cancellation/reset, recurrent checkpoints, vision/cancellation, JSON sampler constraints, reasoning guard, and the repetition-penalty ABI. The final headroom fixture passed in 48.02 seconds.
- Native API suite: **5/5 passed** for chat, generated coding response, vision, streamed tool call, and tool-result continuation. Generated `clamp` code passed six restricted execution cases. See [API results](../evidence/historical/talvik-sampler-native-api.json) and [code checks](../evidence/historical/talvik-generated-code-check.json).
- OpenAI Python SDK 2.48.0 accepted role-first streaming, content, usage and termination. A deliberate request timeout produced `APIError` with `generation_timeout`, rather than an invalid event-line parse. See [streaming](../evidence/historical/talvik-vscode-sdk-live.json) and [error](../evidence/historical/talvik-vscode-sdk-error.json).
- Actual VS Code Ask on Coder 1.5B completed; its 656-token prompt prefill took 7.04 seconds.
- Actual VS Code Plan on `f9b75f3` executed read and search tools and returned three numbered steps. It took roughly 7m24s: a 6,032-token initial prompt completed in 185.354s, then a 6,332-token follow-up crossed the previous 8K allocation boundary and took another 258.802s. This exposed the headroom issue fixed in the installed build.
- Actual VS Code Plan on the installed `37d44a1` passed with read/search tools and three numbered steps, without visible control tags. Saved session inspection confirms completion and 299.860s elapsed. See [IDE Plan check](../evidence/historical/talvik-ide-plan-headroom.json). First request: 224.525s; continuation: 74.263s. The 6,034-token initial and 6,349-token follow-up prompts shared a 16K allocation, with no rebuild for the tool result. Total backend time was 298.788s (about five minutes), compared with 444.156s previously. The outputs/tool selection differed, so this is a live workflow comparison, not a controlled throughput benchmark.
- Actual VS Code **Talvik Coding** on `37d44a1` passed the streamed read-file round trip. The first request took 276.563s; the tool-result continuation took 24.413s. Saved session inspection confirms completion, one completed `readFile`, the correct README heading, and no visible control tags. Total IDE elapsed time: 301.838s. See [IDE Coding check](../evidence/historical/talvik-ide-coding-verified.json). Editing and terminal execution were intentionally outside this read-only probe.
- Attention API reuse: a 510-token cold request took 12.517s; changing the question reused 500 tokens and completed in 0.680s; a shorter prompt reused 497 tokens and completed in 0.752s. These are short API probes, not large IDE response timings. See [prefix reuse](../evidence/historical/talvik-attention-prefix-api.json).
- One before/after device-selection pair reduced Coder prefill from 14.184s to 12.232s, and Qwen3.5 from 34.716s to 28.214s. These are limited serial comparisons, not an Ollama parity claim.

## Remaining release gates

Cold multi-thousand-token prompts still cost minutes on this Intel CPU. The GPU's desktop acceleration does not establish a qualified inference backend. Vulkan was not enabled after earlier correctness failures. Full built-in Agent, context compaction, long editing/terminal workflows, broader OpenAI API compatibility, schema enforcement and cross-platform packaging still need qualification. The focused read-only checks do not certify autonomous coding quality.

Older reports and archives in this folder are historical builds. The installed binary checksums are recorded in [build identity](../evidence/historical/talvik-vscode-installed-identity.json). Recurring overnight tests remain paused.

Source push confirmed: `512990d..37d44a1` to `origin/feature/qwen-coding-assistants`.

## Repeat the focused checks

In VS Code, start a new chat, choose **Talvik Coding**, and ask it to read the first three lines of README.md and return its first heading. The expected sequence is a file tool invocation followed by the heading, without a JSON wrapper. For planning, choose **Talvik Plan** and request three numbered steps after reading the same file. These presets keep tool inventories bounded, but the first cold request can still take minutes.

For backend qualification, run `scripts/test-talvik-native-api.py` with `TALVIK_TEST_CODER_MODEL=qwen3-1.7b-q4_k_m` while the daemon is idle. Use the existing local authentication setup; never paste the token into a report. Saved [workspace regression](../evidence/historical/talvik-vscode-workspace-tests.log), [strict Clippy](../evidence/historical/talvik-vscode-headroom-clippy.log), [native headroom fixture](../evidence/historical/talvik-vscode-headroom-native.log), and [release build](../evidence/historical/talvik-vscode-release-build.log) logs accompany this report.
