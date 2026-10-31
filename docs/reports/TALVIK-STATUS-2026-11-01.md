# Talvik implementation and qualification status — 2026-11-01

## Outcome and release decision

Talvik has a working local API foundation and a committed native chat-template repair. Dependable VS Code Agent execution and acceptable cold-prompt latency remain unqualified. This is a stabilization milestone, **not an RC or production-readiness claim**. A completed stream or a model saying “building” does not prove a build executed.

Implementation commit: `5d5925b` (`fix(talvik): execute embedded model chat templates for tool calls`), pushed on `feature/qwen-coding-assistants`. The installed CLI still identifies `6d323e79b33c5fbad1cb55b981dd01f980d9397e`; the new native renderer library was built/installed during development, but the Rust release containing its integration has not been installed and live-qualified. This documentation task did not restart the daemon or run inference. See the [identity snapshot](evidence/source-install-snapshot-2026-11-01.json). Source, installed CLI, running daemon and packaged archive identities must be checked independently.

The `talvik-qwen-soak-test` recurring automation was deleted at the user's request. No other matching recurring automation was found. Further qualification is manual; it must not recreate the schedule.

## Product surfaces and evidence limits

| Surface | Implemented / historical evidence | Current limit |
| --- | --- | --- |
| Native HTTP API | Local authentication, model selection, Chat Completions/SSE, usage/errors, model lifecycle, queueing and cancellation/recovery | Per-build fixtures and the published compatibility subset govern claims; full OpenAI/Ollama parity is not complete |
| Model capabilities | Loaded profiles, selected tool parsers, tokenizer controls and matching vision assets | A capability probe is not an arbitrary-model quality guarantee |
| Native inference | CPU text/vision, attention/recurrent cache strategies and bounded context ownership | GPU correctness, crash isolation and broad platform qualification remain open |
| VS Code | Custom endpoint plus focused Coding/read-only Plan presets; earlier Ask and read-tool checks | Current full build-only Agent acceptance pending; model narration is a recorded failure |
| CLI / OID Console | Earlier terminal qualification covers wait/thinking/generating states, color/highlighting, plain/piped output and cancellation; console presentation was simplified | UI evidence belongs to its named historical build, not a fresh acceptance of the new renderer |
| Installation | Canonical per-user `~/.marina` layout, managed local service and historical archive smoke tests | Current archive rebuild, clean install/upgrade and new dependency checks pending |

The exact API inventory is maintained in [OpenAI compatibility](../TALVIK-OPENAI-COMPATIBILITY.md), [Ollama compatibility](../TALVIK-OLLAMA-COMPATIBILITY.md), and the [capability matrix](../TALVIK-COMPATIBILITY-CAPABILITY-MATRIX.md). Historical lifecycle, SDK, queue, terminal and UI evidence is preserved in the reports/evidence index; it has not all been rerun for `5d5925b`.

## User-visible failures and cause

The user observed Copilot compaction errors, “Response contained no choices”, excessive waiting, multiple daemon processes, and an Agent response promising a build without running a terminal tool. Earlier API recovery, stream, service-lifecycle and client configuration work is preserved in the [historical reports](README.md#historical-investigations). Those reports establish behavior only for their named builds.

For the action failure, the native C role/content formatter did not execute the embedded GGUF Jinja tools branch. Approximate tool envelopes and system-message ordering were insufficient: short tool probes passed while full IDE execution or requested scope failed. The latest implementation replaces those heuristics with actual template execution. This is a concrete formatter correction, not proof that it resolves every Copilot compaction error or all model instruction-following failures.

## What changed in 5d5925b

- `sys/native/chat-template.cpp` and its CMake target compile the pinned llama.cpp Jinja engine into private `oid-chat-template`. Versioned ABI 1 passes UTF-8 template/context data; native-owned output has explicit length, paired deallocation and error handling.
- `sys/src/chat_template.rs` retains messages, tools, assistant argument objects, call IDs and tool-result content as structured data until rendering. BOS/EOS come from the actual tokenizer. Supported thinking controls reach template execution.
- HTTP tool-result messages retain original content and correlation IDs rather than becoming explanatory prose. The public HTTP shape stays unchanged.
- Tool availability requires the renderer, a usable template tool probe and a supported output parser. This is protocol support detection, not a model-quality benchmark. Plain chat retains a limited C formatter fallback when applicable.
- Optional recurrent cache boundaries are obtained using a marker render and verified against the real rendered prefix; existing exact token/state checks remain. No model name or Maven command is embedded in backend formatting.
- macOS/Linux/Windows setup and packaging include the renderer dependency. Native macOS compilation was checked; Linux/Windows changes still need their own native qualification.

See [ADR-0035](../adr/0035-native-embedded-chat-templates.md) and [WP-0098](../work-packages/WP-0098-talvik-native-inference-performance.md). Native dependency: llama.cpp `18443257a30c884d5332abb8e7dc43c7ffe42fda`.

## Test evidence for the native template change

| Check | Captured result | Meaning and limit |
| --- | --- | --- |
| Formatting and staged diff checks | Passed before commit | Source hygiene only |
| Workspace tests | 279 passed, 0 failed, 10 ignored | Includes doc tests; ignored fixtures are not passes |
| Strict Clippy | Passed for sys and console with dependent adapter/runtime checks | Does not assert a newly run strict lint of every unrelated workspace crate |
| Qwen3 real embedded template | Passed, 2.33 s | Actual client-before-tools layout |
| Qwen3.5 real embedded template | Passed, 6.82 s | Actual tools-before-client layout |
| Native tool grammar | Passed, 1.23 s | Nonempty tool schema fixture |
| Attention prefix reuse | Passed, 43.67 s | Compatible-prefix correctness and recovery fixture |
| Recurrent checkpoints | Passed, 20.51 s | Cold/warm output equivalence and cancellation recovery |
| Vision and cancellation recovery | Passed, 8.12 s | Explicit thinking disabled in the bounded answer fixture |
| Tokenizer/BOS metadata | Passed, 1.89 s | DeepSeek native fixture without duplicated BOS |
| New release install, live API and VS Code acceptance | Pending | Earlier API passes do not qualify the new integration |

Logs and provenance are in the [evidence manifest](evidence/manifest.json). The earlier isolated unit log contains 24 passes/8 ignored before removal of superseded synthetic tests; it is retained for history, not the final workspace count. The first vision attempt exhausted its small answer budget in reasoning; explicitly disabling thinking made the answer/recovery fixture pass. This does not prove every model supports disabling thinking.

## Actual Agent evidence

| Candidate / scenario | Observed behavior | Acceptance |
| --- | --- | --- |
| Original build-only request | Narration, no terminal; 329.747 s | Failed |
| `f3d51b0`, real IDE retry | `mvn clean install`, exit 0, BUILD SUCCESS; 352.241 s | Execution proved, build-only scope failed because clean/tests/install ran |
| Later Qwen3 1.7B with concise preset | No tools; 356.675 s | Failed despite preset being loaded |
| `6d323e7`, Qwen3.5 full IDE probe | Cancelled to replace formatter | No pass/fail conclusion on completed task quality |
| `5d5925b`, native embedded Jinja | Native fixtures pass; full IDE run pending | Agent fix not yet qualified |

See [first IDE execution](evidence/historical/talvik-action-first-ide-build.json) and [later narration-only failure](evidence/historical/talvik-action-qwen3-final-ide-failure.json). Historical 4/4 action-selection probes supplied synthetic context/results and never executed commands. Historical 5/5 native API checks verify their respective model/transport paths, not arbitrary IDE work. Rejected XML-function output and failed action-selection reports are retained as failures.

## Ollama comparison and model adaptation

We follow the same serving principle: represent tools structurally, format input for the selected model, parse native tool output, and return calls to the client. VS Code owns approvals, command execution and the next request carrying results. Setting `toolCalling: true` cannot give an unsuitable model reliable tools.

Ollama's [tool-calling documentation](https://docs.ollama.com/capabilities/tool-calling) describes single, parallel, streamed and multi-turn tool flows. Its [model capability selection](https://github.com/ollama/ollama/blob/main/server/images.go) combines configuration, templates and parsers. This source was consulted as a reference; Talvik does not depend on an Ollama daemon. The earlier local study used Ollama v0.35.0, commit `cc4069396f3ad2c370c53eed2e4a42ac13adab84`; online `main` is mutable and is not that pinned study.

Talvik executes the embedded template using its pinned llama.cpp Jinja engine. It has a narrower qualified parser/model/platform set than a general parity claim would require. Architecture-aware cache handling, tokenizer metadata, matching vision assets and conservative capability checks are implemented in slices. Broad model-family acceptance, nested argument-schema validation, crash-isolated workers, cross-model retained-cache ownership and comprehensive client/platform parity remain open. The [original source study](historical/TALVIK-MODEL-ADAPTATION-STUDY-2026-10-31.md) preserves its earlier findings and priorities; several of its then-open template/profile items have since been implemented.

## Models and VS Code presets

| Local model | Intended client use | Qualification boundary |
| --- | --- | --- |
| Qwen3.5 4B Q4_K_M | Current Coding preset candidate; text, vision and tools | Short native/API evidence exists; latest full IDE build acceptance pending |
| Qwen3 1.7B Q4_K_M | Chat and read-only Plan preset | Previous read-file/Plan evidence; failed full build action selection |
| Qwen2.5 Coder 1.5B | Lightweight Ask/chat, client tools disabled | Simple chat passed historically; tool-result continuation failed |
| Qwen2.5 Coder 3B | Coding diagnostic candidate | Build-only scope failed in a probe; not a proven remedy |
| DeepSeek R1 1.5B | Reasoning chat | Not a qualified Agent/tool model |

Preset model display names are local qualification choices, not backend model-name branches. See the [VS Code setup notes](../examples/vscode/README.md). Ask, Plan and Agent must be qualified separately; a read-only preset intentionally cannot execute builds.

## Latency and hardware

This qualification host is an Intel i7-8569U, four physical/eight logical CPU threads, 16 GiB RAM. The Iris GPU exists, but the tested Vulkan path produced incorrect output and is not qualified. The current serving profile is CPU-first with AVX2/FMA and Apple BLAS; GPU presence does not establish usable accelerated inference.

A full Copilot request supplied 59 tools and 22,056 formatted tokens. Profiling showed active prefill work, not merely a hung UI. Other focused presets still produced roughly 9K-token prompts. Cold prefill dominates those requests; a warm short probe cannot represent their latency.

Implemented improvements include incremental decoding, bounded batches, actual control-token parsing, fewer unnecessary logits, attention-prefix trimming, bounded recurrent snapshots, actual CPU-device selection, demand-sized context allocation and growth headroom. The single retained idle context can still be replaced by a utility-model request. These changes do not establish acceptable cold Agent performance.

There **was** an earlier same-model short-prompt Ollama comparison: Talvik cold/repeat wall times 14.675/1.886 s versus Ollama 12.983/0.909 s. Prompt token counts differed (653/654), the output was only a partial reasoning sequence, and these were older builds. See [raw comparison](evidence/historical/talvik-ollama-same-model-comparison.json). There is no controlled comparison of the newly committed renderer and the actual full IDE request; no parity or hardware-optimality claim is justified.

## Remaining gates, in order

1. Build clean `5d5925b` release binaries with the matching renderer, install only when idle, and record CLI/daemon/library hashes and native pin. Confirm one managed daemon and readiness.
2. Run manual live action-selection and native API suites. Cover streaming tool calls, complete arguments, terminal chunks, correlated results, ordinary questions without tools, vision and cancellation recovery. Keep synthetic command results explicitly labelled.
3. In a fresh Talvik Coding conversation, send `build this project, don't check anything. just build`. Require a real terminal call equivalent to `mvn -DskipTests package`, actual exit 0 and BUILD SUCCESS, tests skipped, and no inspection/edit/clean/install actions. Never equate narration with execution.
4. Requalify Ask and read-only Plan; retain real request/command/result evidence without credentials. Compare full cold/warm IDE latency against the same model/settings/request on Ollama.
5. Close API contract gaps and qualify SDKs, packaging, upgrade/recovery and native dependencies on every advertised OS/architecture. Worker crash isolation and dependable performance remain release work.

No new RC tag, production release or updated release archive was created by this change. Older tarballs retain their old build identities. Use the [manual runbook](TALVIK-QUALIFICATION-RUNBOOK.md) to continue.
