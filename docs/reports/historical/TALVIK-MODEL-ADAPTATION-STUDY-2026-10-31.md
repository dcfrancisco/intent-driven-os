> Historical snapshot imported on 2026-11-01. Build identities, pass claims, pending work, installed versions, and push status below describe their original run, not the latest source. See the [current status report](../TALVIK-STATUS-2026-11-01.md). Local-only archive/image references may require the original artifact directory.

# Talvik model adaptation: first implementation and source study

> Current installed build: `a480958`. See [loaded profiles and output adapters](TALVIK-LOADED-PROFILES-REPORT-2026-10-31.md) for current behavior and exact test status. Earlier build identities and measurements below are historical.

First implementation is committed and installed as clean build `6f3f9bdb4c3813716928476c4fca011fe5851eab` on `feature/qwen-coding-assistants`. All four installed binaries match the clean release build. One managed daemon is ready; normal logging is restored, with no model or credential changes.

Implemented: recurrent static checkpoint boundaries come from structured message roles rendered by the loaded model's selected template, with exact text and token validation. The runtime no longer searches for ChatML role strings. Unsupported, merged, empty or incompatible prefix renders omit the optional checkpoint safely. Full prompt rendering errors still fail explicitly; native buffer lengths are validated before UTF-8 conversion.

Validation: 266 workspace tests passed; strict Clippy, formatting, diff checks and clean release build passed. All six real-model native fixtures passed (three text, grammar, recurrent cold/warm/reset/cancellation, vision). Live installed API checks passed 5/5, including streamed tool call and tool-result continuation; API recovery passed 14/14. Renderer tests cover ChatML, Llama 3, DeepSeek, Vicuna, Gemma fallback, multi-turn history, delimiter text inside user content, unsupported templates and token-boundary mismatches. Renderer tests are not full inference qualification of every named family. Cache acceptance passed 4/4: identical repeated/changed-user answers, exact token reuse, context invalidation and timing decomposition. Final service checks show ready, zero active requests and zero pending requests.

Machine-readable evidence: `talvik-model-adaptation-qualification.json`, `talvik-model-adaptation-installed-identity.json`, `talvik-adaptation-native-api.json`, `talvik-adaptation-recovery.json`, `talvik-adaptation-prompt-cache.json`.

Official Ollama source checked out at `/tmp/talvik-ollama-source`, tag `v0.35.0`, commit `cc4069396f3ad2c370c53eed2e4a42ac13adab84`, matching the installed version. This is a shallow, detached checkout. MIT license at `LICENSE`; preserve required notices if code is subsequently reused. Talvik remains independently executable.

## What the reference actually does

- `server/images.go:153`: capability discovery combines configuration, GGUF metadata, projector presence, selected template, output parser and explicit support restrictions. Metadata alone does not establish working tool or vision behavior.
- `server/images.go:184`: architecture, pooling, vision/audio metadata and template information inform advertised capabilities.
- `server/images.go:431`: parsers declare tool/thinking handling. Some family-specific exceptions remain; Ollama is not one universal formatter/parser for every model.
- `llm/llama_server.go:259`: tokenizer/BOS metadata affects input preparation, with architecture-specific corrections where necessary.
- `server/sched.go:206`: reuse checks runner compatibility; scheduling includes lifecycle, residency, context, memory fit and expiry.
- `server/sched.go:502`: concurrency is bounded, including explicit architecture/family restrictions.
- `llm/llama_server.go`: native serving is delegated to a llama-server runner in this version. Therefore remaining cache behavior must also be traced into the exact bundled engine, rather than attributed solely to the Go HTTP layer.

## Current Talvik findings

- The recurrent checkpoint decision already uses native `llama_model_is_recurrent` / `llama_model_is_hybrid`; it does not select by model filename.
- The original static-prefix boundary searched for `<|im_start|>user\n`. Commit `6f3f9bd` replaces it with a rendering of structured messages before the last user turn, using the selected model template and exact text/token validation.
- Full matching recurrent prompt reuse remains available. The additional static-prefix optimization now accepts only a verified prefix derived from the actual template; unsupported, empty, rewritten or token-incompatible partial renders fall back safely.
- Text generation uses the embedded template, but tools are currently represented through Talvik instructions and a limited output convention. This is not equivalent to complete native template/parser support across families.
- `NativeModel::supports_tools` currently checks a couple of template substrings. This is a hint, not sufficient evidence of a complete round trip.
- Vision depends on usable matching projector assets and a supported backend. A model name or template mentioning images must not be enough to advertise working vision.

## Implementation priorities

1. Build an immutable loaded-model profile from artifact metadata, tokenizer/template information, backend support and matching auxiliary assets. Public capabilities should reflect supported execution, with conservative unknown/unsupported behavior.
2. Render structured messages, tools and media through the selected template. Derive safe cache checkpoints from verified rendered token boundaries, not hard-coded role strings. If a safe boundary cannot be established, fall back to full-prompt reuse or no reuse.
3. Select cache policy by backend/architecture support: attention trimming, recurrent checkpoint restore, or conservative invalidation. Include artifact/template/context/native identities and request compatibility in reuse decisions.
4. Isolate format-specific tool/reasoning parsing behind explicit adapters. Such adapters may be necessary; model-name checks should not be scattered through scheduling and HTTP code. Validate split streaming boundaries and tool-result continuation.
5. Make context, memory admission, auxiliary assets, batching and residency derive from the loaded profile plus actual device capabilities. Retaining main-model cache across utility requests requires explicit admitted memory ownership.
6. Qualify classes of behavior: transformer chat, hybrid/recurrent chat, embeddings, vision/projector, tools and reasoning. Include multiple template families, renamed artifacts, unsupported architectures and misleading metadata. Model fixtures remain named for reproducibility; runtime behavior must not depend on those names.

## Installed cache acceptance

Temperature zero, context 8192, model Qwen3.5 4B; all answers `PERF_OK`. Cold references change context size without unloading weights. These are short synthetic requests, not full VS Code Agent/Plan qualification.

| Request | Prompt evaluation | Wall time | Reused tokens |
|---|---:|---:|---:|
| cold | 32.010 s | 32.753 s | 0 |
| repeat | 0.389 s | 1.246 s | 750 |
| changed_user | 1.069 s | 1.763 s | 736 |
| changed_user_cold_reference | 30.129 s | 30.761 s | 0 |

The changed-user request took 1.763 seconds cached versus 30.761 seconds cold, with identical output. Host timing variation prevents attributing any further speedup over the preceding cache fix to this refactor. This slice establishes model-template-neutral boundary selection and preserves measured reuse.

## Acceptance boundary

Loading a supported model should configure the appropriate runtime path without users editing model-specific code. A valid GGUF is not automatically supported by every backend. Unsupported architecture, missing projector, unsupported tool parser or incompatible request should return an actionable structured error, never guessed capability or fabricated success.

Installed commit `6f3f9bd` includes the prior latency fix and closes the hard-coded static cache-boundary assumption; capability profiles, complete template/tool rendering, parser selection and cross-model cache ownership remain open. This is not an RC promotion.

Git source is clean and committed locally. GitHub push remains pending because HTTPS credentials are unavailable; this slice did not retry the unchanged authentication failure. The older archive remains the `e91dc0f` candidate and has not been relabeled as this build. No recurring tests were enabled.
