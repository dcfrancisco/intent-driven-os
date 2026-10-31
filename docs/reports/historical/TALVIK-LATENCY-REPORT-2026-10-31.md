> Historical snapshot imported on 2026-11-01. Build identities, pass claims, pending work, installed versions, and push status below describe their original run, not the latest source. See the [current status report](../TALVIK-STATUS-2026-11-01.md). Local-only archive/image references may require the original artifact directory.

# Talvik latency investigation — 2026-10-31

> Current installed build: `a480958`. See [loaded profiles and output adapters](TALVIK-LOADED-PROFILES-REPORT-2026-10-31.md) for current behavior and exact test status. Earlier build identities and measurements below are historical.

> Current installed build: `6f3f9bd`. See [model adaptation implementation](TALVIK-MODEL-ADAPTATION-STUDY-2026-10-31.md) for current qualification. Measurements and package identities below remain evidence for their stated earlier commits.

Status: **measured optimization installed and qualified locally; full IDE latency remains open.**

Clean commit `8c64517e5eae129652baad5add7e1ce525fbaca4`, branch `feature/qwen-coding-assistants`. All four installed binaries match that clean release build. One managed daemon is ready; private credentials are unchanged and temporary profiling is disabled. The earlier `e91dc0f` archive/report describe the preceding candidate, not this installed build.

## Causes measured

1. **The VS Code request is large.** A read-only Agent request carried 59 tools (75,220 serialized bytes), a 22,430-byte system message, and two user messages. Talvik formatted 22,056 input tokens. The first 512 tokens took 31.4 seconds; 1,024 took 52.565 seconds. The bounded profiling request was canceled after 100 seconds. This is not a completed Agent qualification.
2. **CPU prompt evaluation dominates.** An idle 769-token baseline spent 26.371 seconds on prompt evaluation and about 0.611 seconds on the remaining inference, plus 10.338 seconds loading. CPU samples show quantized matrix multiplication and dequantization. AVX2, FMA, repacking, and Apple Accelerate are enabled.
3. **Qwen3.5 recurrent cache restoration was failing.** Native logs showed failed cache trimming followed by a complete cache clear on repeats. Reprocessing the same prefix is avoidable work.
4. **Model switching loses the main cache.** The existing memory policy retains one idle native context across resident models. VS Code utility requests to Coder can discard the main Qwen context. This remains open.
5. **Memory pressure contributes, but did not explain most delay.** Unloading two idle models stopped measured swap-outs while prefill changed only from 32.094 to 31.768 seconds. Four-thread and larger-batch experiments showed no reliable speedup; defaults remain unchanged.

The host is a four-core/eight-thread Intel i7-8569U with 16 GiB RAM. Ollama's own local startup log identifies CPU inference and zero usable VRAM. An Intel GPU being present does not establish an accelerated, correct inference path. Talvik's earlier Vulkan path produced incorrect output and remains unqualified.

## Implemented optimization

- Parse actual template control tokens for text generation instead of splitting role delimiters into ordinary tokens.
- Preserve up to two bounded recurrent partial-state checkpoints, including a matching static system/tool prefix. Validate exact token prefixes and native cache position before reuse. Reset, cancellation, failures, context changes and unload discard the state. Snapshots are capped at 64 MiB each and 128 MiB total; the measured Qwen state is about 50 MiB per snapshot.
- Request logits only for the final prefill batch and reserve one output. The Qwen compute-buffer reservation fell from roughly 503 MiB to 71–77 MiB. This is not total process memory and excludes checkpoint storage.
- Separate decode time/output throughput from prompt evaluation in API metrics.
- Add opt-in count/timing diagnostics, off by default; no raw prompts or credentials logged.

A live regression caught an empty static prefix for user-only chats. The prefix is now skipped and covered by the real recurrent-model fixture.

## Evidence

- `talvik-ide-payload-profile.json`: actual VS Code request sizes and bounded progress.
- `talvik-prefill-sample.txt`: native CPU sampling.
- `talvik-memory-latency-bench.json`: resident-model and swap experiment.
- `talvik-thread-benchmark.jsonl`: isolated thread/batch comparison; no reliable gain.
- `talvik-checkpoint-api-benchmark.json`: initial cache candidate. Identical changed-user answer: 43.784 s cold versus 1.494 s cached prefill; 48.855 s versus 2.565 s total wall time. These are short synthetic prompts, not the 22K-token Agent workload.

## Final clean-build results

The Qwen benchmark uses the same resident model, context 8192, temperature zero and a 751–753-token prompt. Context-size changes establish a cold reference without reloading weights. All four answers were `PERF_OK`.

| Request | Prompt processing | Total wall time | Reused input tokens |
|---|---:|---:|---:|
| cold | 31.736 s | 32.510 s | 0 |
| repeat | 0.459 s | 1.619 s | 750 |
| changed_user | 1.394 s | 2.556 s | 736 |
| changed_user_cold_reference | 31.765 s | 32.721 s | 0 |

The changed-user cached request took 2.556 seconds versus 32.721 seconds for its cold reference (about 12.8× faster). This establishes a short-prompt cache benefit, not cold-prefill acceleration or an end-to-end VS Code Agent pass.

### Ollama comparison

Both runners used the exact same installed DeepSeek-R1 1.5B weight file (Talvik's file is a symlink to Ollama's blob), the same user message, context 4096, eight CPU threads, temperature zero, seed 42 and twelve output tokens. They ran serially. Ollama used 654 input tokens; Talvik used 653, reflecting a framing/BOS difference. This is one pair of runs, not a statistically controlled proof of every remaining cause. Both emitted the same initial reasoning text; the twelve-token budget was intentionally too short to qualify answer quality.

| Runner/request | Prompt processing | Output decode | Total wall time |
|---|---:|---:|---:|
| talvik cold | 12.490 s | 2.082 s | 14.675 s |
| talvik repeat | 0.383 s | 1.468 s | 1.886 s |
| ollama cold | 9.141 s | 0.932 s | 12.983 s |
| ollama repeat | 0.062 s | 0.827 s | 0.909 s |

Ollama is faster in this probe. Both logs confirm CPU, AVX2/FMA/Accelerate/repack, eight inference/batch threads, context 4096 and batch 512. Ollama's cold wall time includes 2.863 seconds loading; Talvik's model was already resident, while its prompt cache was cold. The worker-pool hypothesis was checked in an isolated native A/B/A/B test: temporary-pool prefill 5.986/6.502 s versus persistent-pool 7.211/7.124 s; 24-token decode 1.786/1.912 s versus 1.743/2.166 s. The greedy final token matched. This did not demonstrate a reliable improvement, so no worker-pool change was installed. The direct harness had one graph split versus 55 in the serving processes; its absolute times are not API parity measurements. Remaining native serving differences and host variation need further attribution. No performance parity claim is made.

### Validation

- Workspace: 262 passed; six native-dependent tests ignored in the ordinary suite.
- All six native-dependent fixtures run explicitly: text/template controls, large prompt and reuse/reset/cancel behavior, grammar, vision, recurrent checkpoints and user-only Qwen prompt.
- Clean installed HTTP checks: 5/5 (chat, coding, vision, streamed Agent tool call, tool-result continuation).
- API failure/recovery checks: 14/14.
- Prompt-cache/timing acceptance: 4/4; exact outputs, matching-prefix reuse, context invalidation, phase decomposition and decode-throughput consistency.
- Strict Clippy, formatting and diff checks passed with Rust 1.88.0 and pinned native libraries.

Machine-readable evidence: `talvik-latency-qualification.json`, `talvik-latency-installed-identity.json`, `talvik-latency-native-api.json`, `talvik-latency-recovery.json`, `talvik-latency-final-prompt-cache.json`, `talvik-ollama-same-model-comparison.json`.

## Remaining product work

Cold large Agent prompts and actual end-to-end Agent/Plan completion remain unqualified. Warm caching cannot remove the first prefill. Final checks left the managed daemon ready with zero active/pending requests. All benchmarking is complete; no recurring tests were enabled. Cross-model cache retention must respect memory admission; reducing unnecessary tools/context and qualifying a faster backend/model route remain useful next steps. No RC claim follows from this benchmark. GitHub push remains blocked by unavailable credentials; no token is stored in this report.
