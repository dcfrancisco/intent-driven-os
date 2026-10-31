> Historical snapshot imported on 2026-11-01. Build identities, pass claims, pending work, installed versions, and push status below describe their original run, not the latest source. See the [current status report](../TALVIK-STATUS-2026-11-01.md). Local-only archive/image references may require the original artifact directory.

# Talvik loaded model profiles and output adapters — 2026-10-31

Status: installed as clean commit `a480958d52cc7ab53cc842a9748258a0155a05ae` on `feature/qwen-coding-assistants`. All four installed binaries match the release build. One managed daemon is ready; credentials and startup configuration are unchanged. A new user IDE request interrupted the final idle-only recovery/cache/live-SDK sequence. The implementation is installed and its new feature checks passed; the remaining final checks are deferred pending the user's cancellation/defer choice.

## Implemented behavior

The native adapter constructs an immutable profile at load from actual architecture, decoder, tokenizer, template and initialized projector facts. Filenames and family names do not select execution behavior. Profile data is cached separately from the inference mutex and removed on unload. `GET /v1/models` exposes additive `runtime_profile` data, with `null` for unloaded/unknown models; discovery does not load weights. Ollama `/api/show` separates loaded supported capabilities from registry declarations.

Profiles select attention or recurrent checkpoint reuse, tokenizer-driven BOS insertion exactly once, tool instructions and output parsers. Public literal tokenization is unchanged. Tool protocols now include JSON tool calls and function/parameter blocks. Standalone leading bare/fenced JSON calls are bounded compatibility forms; code examples later in ordinary prose remain content. Typed function parameters preserve schema-declared strings and parse directly declared numeric/boolean/object/array types. Malformed, duplicate, oversized or unsupported forms fail explicitly. The daemon does not execute tools.

Recognized leading reasoning is separated before tool parsing, including markers split across chunks. Thought text cannot become a tool proposal. Chat Completions uses `message.reasoning_content` and SSE `delta.reasoning_content`; Ollama Chat uses `message.thinking`. Template-provided reasoning prefixes and supported thinking switches are honored. Unsupported explicit switches return 501. A global default cannot turn off a model without such a control. Full token usage still includes reasoning; no reasoning-token count is invented.

## Observed loaded profiles

These are artifact/backend facts for the installed local fixtures, not hard-coded runtime rules or guarantees of model answer quality.

| Registered model | Architecture | Cache | Tool parser | Reasoning | Thinking switch | BOS |
|---|---|---|---|---|---|---|
| qwen2.5-coder-3b-instruct-q4_k_m | qwen2 | attention | xml_json | unavailable | False | False |
| qwen3.5-4b-Q4_K_M | qwen35 | recurrent_checkpoint | xml_function | think_tags | True | False |
| deepseek-r1-1.5b | qwen2 | attention | unavailable | think_tags | False | True |

The vision capability is enabled only for the Qwen3.5 fixture's successfully initialized matching projector. Unloaded artifacts and absent/unavailable projectors are not promoted from filename hints. The native adapter does not implement correct pooled/encoder embedding admission and extraction, so it reports any embedding declaration separately and returns unsupported rather than advertising or returning an unqualified vector.

## Validation and pending checks

- Workspace: 273 passed, eight native-dependent fixtures ignored in the ordinary run.
- All eight native fixtures explicitly passed: text/template controls, physical-batch overflow, context reuse/reset/cancel, grammar, profile inspection while the inference lock is held, profile removal on unload, recurrent equivalence/recovery, vision recovery, metadata/BOS behavior (several assertions share fixtures).
- Strict Clippy, formatting, diff checks and clean release build passed with Rust 1.88.0.
- Clean installed baseline API: 5/5 — chat, coding, vision, streamed function/parameter call and tool-result continuation.
- New profile/parser acceptance: 8/8 — loaded profiles; Coder JSON tool calls in nonstream/SSE; reasoning in nonstream/SSE; DeepSeek metadata/reasoning; Ollama thinking and unsupported switches; discovery during inference. The discovery probe completed in 9 ms on the first candidate and the final-run measurement is in the JSON report.
- The thinking probes intentionally use a 96-token output budget. Qwen exhausted it while still reasoning; these checks qualify separated transport and length termination, not completion of a reasoned answer. Default thinking-off chat returned `4`. DeepSeek's short-budget answer was also length-limited. Full IDE Agent/Plan behavior is not qualified by these short probes.

Evidence: `talvik-loaded-profiles-qualification.json`, `talvik-loaded-profiles-installed-identity.json`, `talvik-profiles-native-api.json`, `talvik-profiles-acceptance.json`, `talvik-profiles-recovery-interrupted.json` and `talvik-profiles-sdk-schema.json`. The Coder diagnostic found a fenced JSON call and directly motivated the tested compatibility handling; no user files were read by the emitted tool proposals.

Eight recovery cases passed before a new user request acquired the scheduler; the fixture stopped at its idle precondition. This is recorded as an interrupted run, not a complete 14-case pass or a demonstrated scheduler regression. The new request was not canceled without permission. Six remaining recovery cases, the final API cache benchmark and live SDK procedure are deferred. Two OpenAI Python SDK 2.48.0 schema fixtures preserve the new reasoning fields; those are local schema checks, not live HTTP acceptance.

## Remaining product work

This completes the loaded-profile and currently recognized output-adapter slice. Arbitrary Jinja/tool-template rendering, additional tool/reasoning formats, admitted pooled embedding execution, cross-model/session cache retention and large cold IDE prompt latency remain open. Training context limits are metadata, not available-memory guarantees or a replacement for Talvik's public context cap/admission policy. No universal-model, GPU, production-ready or RC claim is made. Other platforms and clean-machine release/install qualification remain separate gates.

The change is committed locally. GitHub push remains blocked by missing credentials; no unchanged authentication retry was performed. No recurring tests were enabled. The older `e91dc0f` release archive remains historical and was not relabeled as this build.
