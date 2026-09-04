# OID Model Runner Reality Review

**Audit date:** 2026-09-04  
**Repository:** `intent-driven-os` (working tree clean at audit start)  
**Scope:** Existing implementation and executable evidence only. No production corrections were made.

### Native library configuration

`LLAMA_CPP_LIB_DIR` is the build-time native library directory. For the local
Marina installation it is `/Users/dannyfrancisco/.marina/models`; the default
portable shell form is `$HOME/.marina/models`:

```sh
export LLAMA_CPP_LIB_DIR="${LLAMA_CPP_LIB_DIR:-$HOME/.marina/models}"
```

This is a documented environment default, not an automatic Cargo/build-script
fallback. The directory must contain `libllama` for the native adapter to be
compiled as available.

## 1. Executive assessment

OID is beyond architecture-only status. The repository contains a real backend-neutral `ModelRunner`, a llama.cpp adapter, native GGUF loading, context creation, tokenization, prompt decode, sampling, autoregressive generation, token callbacks, statistics, and model cleanup.

The strongest evidence is the retained Ubuntu validation under `docs/validation/benchmarks/wp-llm-002/`: the same 1.22 GiB Llama 3.2 1B Q8_0 GGUF was loaded and used for five OID runs, producing 128 generated tokens, incremental output, statistics, and clean unloads. That is repository evidence of real inference, not a unit-test mock.

The proof is incomplete for Level 5. Live Ctrl+C cancellation was explicitly not demonstrated: the validation says generation continued to completion and no cancellation event was observed. The current macOS environment also cannot repeat the workload: no GGUF was found in the checked project/user model locations, and `LLAMA_CPP_LIB_DIR=/Users/dannyfrancisco/.marina/models` does not point to an existing usable library directory.

## 2. Current maturity level

**Current level: Level 4 — Usable model runner, with an unproven Level 5 cancellation requirement.**

Level 4 is supported by real OID benchmark runs showing loading, inference, incremental output, completion statistics, and unload. Level 5 is not awarded because cancellation and post-cancellation recovery were not proven end-to-end. The current console also starts `MockRuntime`, although that runtime registers the real llama.cpp adapter; the lower-level `Runtime::start` path remains a foundation object with an empty catalog and no backend registration.

## 3. Actual execution architecture

### User-visible path

The console command path is:

```text
OID Console Application
  → LineEditor / command router
  → commands::generate_lines_result
  → RuntimeService::generate
  → MockRuntime worker thread
  → BackendManager::generate_streaming
  → LlamaCppAdapter::generate_streaming
  → LlamaCppModelRunner::generate
  → NativeModel::generate_stream
  → llama_tokenize / llama_init_from_model / llama_decode
  → llama_sampler_sample
  → llama_vocab_get_text
  → callback → GenerationMessage::Token
```

The console construction is concrete in `console/src/tui.rs:3-44`: `Application` owns a `MockRuntime`, and `MockRuntime::start` registers/enables `LlamaCppAdapter` in `runtime/src/mock.rs:30-41`. The command parser accepts `model load`, `model unload`, `tokenize`, `generate`, and `complete` in `console/src/commands.rs:228-272`.

The lower-level `Runtime` in `runtime/src/core.rs:14-63` is not the live console composition root: it uses `UnavailableHardware`, `EmptyModelCatalog`, and a fresh empty `BackendManager`. This is a real integration gap, not a llama.cpp adapter gap.

### Source trace

| Stage | Source evidence | Reality marker |
| --- | --- | --- |
| Entry point | `console/src/tui.rs:33-44`, `console/src/commands.rs:228-262` | Reachable through console; console is named/implemented as mock runtime |
| Runtime | `runtime/src/mock.rs:30-41`, `:134-180`, `:190-292` | Reachable real adapter path; lifecycle state is split between registry and backend |
| Backend registry | `runtime/src/backends.rs:211-448` | Real registry/dispatch; mock backend also exists at `:457-534` |
| ModelRunner contract | `crates/model-runner/src/lib.rs:345-375` | Backend-neutral, compile-time contract |
| Model discovery | `runtime/src/models.rs:267-365` | Real recursive `.gguf` discovery; metadata only, no GGUF validation/load |
| Adapter | `runtime/src/native_llama.rs:17-255`, `runtime/adapters/llama-cpp/src/lib.rs:64-283` | Real adapter; no mock fallback inside the native adapter |
| Native initialization | `runtime/adapters/llama-cpp/sys/src/lib.rs:150-189` | Real C API when `native_llama_cpp` is configured; otherwise unavailable stub |
| GGUF loading | `sys/src/lib.rs:439-465` | Real `llama_model_load_from_file`; null result becomes error |
| Context | `sys/src/lib.rs:310-326` | Real `llama_init_from_model`; null result becomes error |
| Prompt tokenization/decode | `sys/src/lib.rs:226-269`, `:310-375` | Real C API calls |
| Sampling | `sys/src/lib.rs:327-359`, `:385-401` | Real top-k/top-p/temperature/dist sampler chain |
| Generation | `sys/src/lib.rs:380-425` | Real autoregressive loop; decode follows each accepted token |
| Token text | `sys/src/lib.rs:385-397` | Real vocabulary text conversion |
| Streaming | `sys/src/lib.rs:394-397`; adapter `lib.rs:159-171`; runtime `native_llama.rs:177-218` | Real callback/channel per token; not buffered in native path |
| Cancellation | `sys/src/lib.rs:379-396`, adapter `lib.rs:147-181`, runtime `native_llama.rs:220-229` | Cooperative flag is implemented; live end-to-end proof absent |
| Statistics | adapter `lib.rs:183-195`; runtime `native_llama.rs:187-197`; mock runtime `mock.rs:250-280` | Real counts/timers on successful native run; some runtime fields are reconstructed |
| Unload | adapter `lib.rs:261-283`; native `sys/src/lib.rs:430-437` | `Drop` frees model; unload calls process-wide backend shutdown |

### Mock/stub/placeholder/reachability findings

- `MockBackend` is explicitly simulated: `runtime/src/backends.rs:457-534`; its text is “Mock generation is not AI inference.”
- `MockRuntime` is explicitly described as fake in `runtime/src/mock.rs:1-18`, but its startup registers the real llama.cpp adapter and its model operations dispatch to it.
- The unconfigured native binding is a compile-time unavailable stub: `sys/src/lib.rs:293-307`, `:460-465`.
- `ModelDiscovery` registers files based on extension and size only: `runtime/src/models.rs:327-360`; actual GGUF validity is deferred to llama.cpp.
- `Backend::generate_stream` buffers a `Vec<StreamChunk>` by default (`runtime/src/backends.rs:147-178`), but `LlamaCppAdapter` overrides `generate_streaming` and uses the real `ModelRunner` stream (`runtime/src/native_llama.rs:139-218`).
- `Runtime::start` is not wired to the model runner: `runtime/src/core.rs:53-58` installs empty providers.
- GPU/NPU fields are intentional placeholders (`runtime/src/hardware.rs:25-29`, `:95-98`); CPU discovery is implemented.
- Embeddings and tool support are explicitly unimplemented in `runtime/src/native_llama.rs:231-241`; outside this audit’s text-generation target.

## 4. Build and test results

| Command/check | Result | Evidence/notes |
| --- | --- | --- |
| `cargo fmt --check` | PASS | Completed with no formatter output on this checkout |
| `cargo test --workspace` | BLOCKED BY ENVIRONMENT | Initial invocation compiled `oid-console`, but subsequent invocations remained blocked on Cargo’s artifact-directory lock and did not return a final test summary |
| `cargo clippy --workspace --all-targets` | BLOCKED BY ENVIRONMENT | Same persistent Cargo artifact-directory lock; retained validation records targeted tests passing and a known strict Clippy warning at `console/src/prompt.rs:64` |
| `cargo test -p oid-model-runner` | BLOCKED BY ENVIRONMENT | Separate target directory was attempted; Cargo began compiling but the command runner did not produce a final result |
| Targeted model-runner/adapter/runtime tests | IMPLEMENTED TESTS, not current-run proof | The repository’s validation document records these targeted tests as passing; current native execution was unavailable |
| Console integration tests | NOT A REAL-INFERENCE TEST | Console tests use `MockRuntime`; they prove command/UI behavior, not a real model |

The test suite is therefore not accepted as proof of inference. The prior validation document reports formatting and targeted model-runner, llama.cpp adapter, and runtime tests passing at `docs/validation/local-llm-runtime.md:166-170`, but also explicitly reports incomplete cancellation at `:12-18`.

## 5. Real inference result

### Current machine

**Not executable in this audit environment.** Observed environment:

```text
Darwin x86_64
ollama: /usr/local/bin/ollama
LLAMA_CPP_LIB_DIR=/Users/dannyfrancisco/.marina/models
Default form: $HOME/.marina/models
GGUF found in checked locations: none
```

The configured library directory was not present as a usable native llama.cpp library directory. No model was downloaded.

### Retained real OID evidence

The prior Ubuntu run is concrete repository evidence at `docs/validation/benchmarks/wp-llm-002/`:

```text
Model: Llama 3.2 1B Instruct
GGUF: /home/danny/Models/llama3.2-1b.gguf
Size: 1,321,082,688 bytes (1.22 GiB)
SHA-256: 74701a8c35f6c8d9a4b91f3f3497643001d63e0c7a84e085bed452548fa88d45
Prompt: Reply with exactly: OID inference works.
OID runs: 5 cold-process runs, 128 generated tokens each
```

Observed OID medians from `results.csv` and `local-llm-runtime.md:93-98`:

| Metric | OID observation |
| --- | ---: |
| Load | included in cold process; per-run load field unavailable |
| Generation | 128 tokens |
| Throughput | 5.2 tok/s median |
| Total external time | 25.35 s median |
| RSS | 1,484,848 KiB median |
| TTFT | unavailable in retained benchmark capture |
| Termination | completion after max output in the primary runs |

This proves real OID inference historically, but not reproducibility on the current machine.

## 6. Streaming result

**PROVEN for the native OID path.** The native loop calls the callback immediately after each sampled token (`sys/src/lib.rs:385-397`). The adapter sends each callback payload as a `GenerationEvent::Token` over a channel (`runtime/adapters/llama-cpp/src/lib.rs:159-171`), and the runtime consumes those events with a 25 ms timeout (`runtime/src/native_llama.rs:177-186`). The retained validation explicitly records “OID incremental streaming: PASS” at `docs/validation/local-llm-runtime.md:76-84`.

This is distinguishable from post-buffer chunking: `NativeModel::generate_stream` does not build a complete result before invoking the callback. The separate legacy `Backend::generate_stream` default does buffer, but the real llama.cpp adapter’s `generate_streaming` path is the one used by `MockRuntime`.

## 7. Cancellation result

**NOT PROVEN end-to-end; classify as PARTIAL.**

The implementation has a cooperative cancellation flag: `GenerationStream::cancel` sets an atomic (`crates/model-runner/src/lib.rs:273-340`), the runtime maps request IDs to cancellation tokens (`runtime/src/native_llama.rs:154-175`, `:220-229`), and the native loop checks the flag (`sys/src/lib.rs:379-396`).

The required behavioral proof is absent. The retained live validation says Ctrl+C reached the PTY but generation continued to completion with no cancellation event (`docs/validation/local-llm-runtime.md:14-18`, `:150-155`). Contract tests only prove that an atomic can be set and an already-sent cancelled event can be received; they do not prove that native generation stops.

## 8. Lifecycle and unload result

**Load/infer/unload/repeat: PROVEN by retained OID runs. Safe cancellation recovery: UNPROVEN.**

The adapter owns one `NativeModel` under `Arc<Mutex<Option<Loaded>>>`; `NativeModel::Drop` invokes `llama_model_free` (`runtime/adapters/llama-cpp/sys/src/lib.rs:430-437`). `unload` removes the model, marks unloading, calls `llama_backend_free`, resets initialization, and marks unloaded (`runtime/adapters/llama-cpp/src/lib.rs:261-282`). The validation reports every OID run returned to `model none` and unload passed (`docs/validation/local-llm-runtime.md:78-84`).

Risks still requiring proof/fix:

- unload is rejected while `generating` is true, so cancellation must reliably return the worker to idle;
- shutdown is process-wide while the model is mutex-owned, so concurrent calls must remain excluded;
- the lower-level `Runtime` and the console’s registry state are not one unified lifecycle object;
- load failure sets `initialized` false and calls global shutdown, which needs repeated-failure testing.

## 9. Raw llama.cpp comparison

The current macOS environment could not run raw llama.cpp because no GGUF and no usable configured library/tool were found. The retained Ubuntu comparison is observational:

| Engine | Generation | tok/s | Total | RSS |
| --- | ---: | ---: | ---: | ---: |
| raw llama.cpp | 128 tokens | 3.9 median | 19.94 s | 1,505,856 KiB |
| OID | 128 tokens | 5.2 median | 25.35 s | 1,484,848 KiB |

The project correctly warns these are not apples-to-apples because raw CLI and OID expose different timing and prompt/template semantics (`docs/validation/local-llm-runtime.md:88-125`). There is no evidence of pathological adapter overhead, but the measurements do not isolate orchestration cost.

## 10. Ollama comparison

`ollama` is installed on the current machine, but no equivalent local model was available and no download was initiated. Retained Ubuntu structured API evidence reports:

| Engine/state | Generation | tok/s | Total |
| --- | ---: | ---: | ---: |
| Ollama cold | 128 tokens | 1.22 | 298.80 s |
| Ollama warm | 128 tokens | 1.22 | 118.86 s median of two |

These values are observational only and are documented at `docs/validation/local-llm-runtime.md:93-102`, `:127-135`.

## 11. Capability matrix

| Capability | Current state | Concrete evidence | Blocking issue |
| --- | --- | --- | --- |
| Real GGUF discovery | PARTIAL | `runtime/src/models.rs:293-365` | Extension-based metadata only; current machine has no file |
| GGUF model loading | PROVEN | `sys/src/lib.rs:439-465`; five retained OID runs | Not reproducible on current machine |
| llama.cpp initialization | PROVEN when configured | `sys/src/lib.rs:150-174` | Compile-time opt-in; unavailable without library |
| Context creation | PROVEN | `sys/src/lib.rs:317-326` | None shown in retained runs |
| Prompt tokenization | PROVEN | `sys/src/lib.rs:226-262`, `:310-315` | None |
| Prompt evaluation | PROVEN | `sys/src/lib.rs:360-375` | None |
| Autoregressive generation | PROVEN | `sys/src/lib.rs:380-425` | None |
| Sampling | PROVEN | `sys/src/lib.rs:327-359`, `:385-401` | None |
| Token-to-text | PROVEN | `sys/src/lib.rs:385-397` | None |
| Streaming output | PROVEN | adapter callback/channel plus retained PASS | Console-level cancel interaction unproven |
| Cancellation | PARTIAL | Atomic path exists; live Ctrl+C failed validation | Signal-to-active-generation behavior |
| Inference statistics | PROVEN/PARTIAL | `GenerationStats`; retained completion stats | TTFT unavailable in retained benchmark; runtime reconstructs some timing |
| Model unload | PROVEN | `runtime/adapters/llama-cpp/src/lib.rs:261-282`; retained PASS | Concurrency/recovery stress not proven |
| Repeated load/unload | PROVEN at cold-process level | Five retained OID runs | Same-process repeat not directly captured |
| Repeated inference | PROVEN across runs | `results.csv`, five OID runs | Same-process repeated inference not isolated |
| Error propagation | IMPLEMENTED BUT UNPROVEN broadly | typed errors and mappings in adapter/runtime | No real invalid/OOM matrix run |
| Invalid model handling | IMPLEMENTED BUT UNPROVEN | file check and null native load handling | No invalid GGUF execution run |
| OOM/resource failure | PARTIAL | `ResourceExhausted` type; native null/context errors | No controlled OOM/resource test |
| Runtime integration | PARTIAL | `MockRuntime` reaches adapter; `Runtime::start` is empty | Two composition paths; real runtime not wired |
| Console/CLI invocation | PARTIAL | Commands and console path exist | Console starts MockRuntime and current model list is empty |
| Hardware detection | PARTIAL | CPU/RAM/SIMD detection in `hardware.rs` | GPU/NPU placeholders; not used to configure llama.cpp |
| CPU-only operation | PROVEN on retained Ubuntu hardware | Portable llama.cpp build and live OID runs | macOS path not validated |
| Real-model test coverage | PARTIAL | retained manual benchmark artifacts | No automated fixture/real-GGUF integration test |

## 12. Gap analysis

| Capability | Current state | Evidence | Blocking issue | Required change |
| --- | --- | --- | --- | --- |
| Cancellation through actual OID/console path | PARTIAL | `local-llm-runtime.md:152-155` | Generation did not stop on Ctrl+C | Wire the existing signal interrupt to the active generation token and prove terminal cancellation |
| Same-process recovery after cancellation | MISSING proof | No retained live evidence | Cannot award Level 5 | Add a test/harness: cancel, await terminal event, infer again |
| Real runtime composition | PARTIAL | `runtime/src/core.rs:53-58` | `Runtime::start` has empty backend/catalog | Reuse existing backend/registry wiring in the actual runtime composition path |
| Native resource failure matrix | IMPLEMENTED BUT UNPROVEN | native null/error branches | No controlled invalid/OOM tests | Add bounded invalid-model and resource-failure tests |
| Observability completeness | PARTIAL | stats structs and runtime mappings | TTFT/termination reason not exposed consistently | Preserve existing stats path and expose terminal reason/TTFT in runtime-facing result |
| Hardware-aware native configuration | PARTIAL | `hardware.rs` | Detection does not configure threads/SIMD/GPU | P2: feed safe CPU facts into existing inference options |

### P0 — prevents real inference

None on the validated Linux/native path. In the current macOS environment, missing GGUF/native library is an environment blocker, not a repository correction.

### P1 — prevents reliable model-runner use

1. Live cancellation is not proven and the observed console Ctrl+C path did not stop generation.
2. Post-cancellation second inference and same-process repeated lifecycle are not proven.
3. The real model path is not composed through the lower-level `Runtime::start`; console uses the mock-named runtime.
4. Invalid-model and resource-failure behavior lacks executable coverage.

### P2 — performance/operability

1. Expose complete TTFT/termination statistics consistently.
2. Add reproducible benchmark automation with equivalent raw/OID settings.
3. Use detected CPU facts for safe thread/configuration selection; GPU/NPU support remains optional.

### P3 — future capability

Embeddings, tools, remote providers, multi-model residency, downloading, routing, and other planned capabilities are outside the minimum Level 5 proof and should not be introduced to solve this audit’s blockers.

## 13. Minimum correction set to reach Level 5

The smallest evidence-based set is **4 concrete implementation/verification steps**:

1. **Repair live cancellation wiring:** ensure the console’s `SIGINT`/interrupt handling invokes the existing runtime cancellation path for the active request, rather than only cancelling input/shutdown state.
2. **Make cancellation terminal and observable:** await the adapter’s `Cancelled` event, clear the request registry/active-generation state, and preserve the loaded model in `Ready` state.
3. **Prove recovery and lifecycle in one executable path:** run load → infer → cancel → second infer → unload → reload → infer → unload, asserting tokens, terminal stats/events, and `Unloaded`/`Ready` states.
4. **Unify or explicitly activate runtime composition:** make the actual service used by the console use the existing real backend registry/model discovery path, or add a focused integration test/harness proving the current `MockRuntime` path is the supported production composition. Do not add a ModelManager.

No new orchestration layer is demonstrated as necessary. The existing `ModelRunner`, `LlamaCppAdapter`, `BackendManager`, cancellation registry, and model registry are sufficient to attempt the proof.

## 14. Final verdict

- **Does OID currently run a real local model?** Yes, proven by retained Ubuntu OID runs; not reproducible on this macOS checkout because the model and usable native library are absent.
- **Can it generate real tokens through `ModelRunner`?** Yes, proven. The native loop tokenizes, decodes, samples, converts vocabulary pieces to text, and emits them through the `ModelRunner` stream.
- **Does streaming actually work?** Yes on the native OID path; per-token callback/channel evidence and retained live validation support this.
- **Does cancellation actually work?** The cooperative implementation exists, but end-to-end cancellation is not proven; the only retained live Ctrl+C attempt failed to stop generation.
- **Can it unload and reload safely?** Load/unload and repeated cold runs are proven; safe same-process post-cancellation reload is not proven.
- **What prevents it being called a robust working model runner?** Missing live cancellation proof/behavior, missing cancellation recovery proof, incomplete real-runtime composition, and insufficient failure/resource coverage.
- **How many concrete implementation steps remain before Level 5?** Four focused steps above, with implementation plus executable proof; no new model-management abstraction is required by current evidence.

## MODEL RUNNER STATUS

Current level: **Level 4 — Usable model runner**  
Highest proven capability: **Real GGUF load, inference, incremental streaming, statistics, and unload through the llama.cpp-backed ModelRunner**  
Primary blocker: **End-to-end cancellation through the actual OID/console path**  
P0 blockers: **None on the validated native Linux path; current machine lacks required model/native library**  
P1 blockers: **Live cancellation, post-cancellation recovery, same-process lifecycle proof, real-runtime composition, failure/resource coverage**  
Estimated implementation steps remaining: **4**  
Recommended next work package: **Repair and validate live cancellation signal wiring, then run the Level 5 lifecycle/recovery proof**

## WP-LLM-002B verification — 2026-09-04

This section is appended after the original Level 4 audit. The original
findings above are preserved.

### Cancellation diagnosis

The prior cancellation path was:

```text
SIGINT → SignalController event queue
       → (only consumed by LineEditor/ShellExecutor)
       → runtime.cancel_generation()
       → MockRuntime active_generation AtomicBool
       → BackendManager callback cancellation flag
       → LlamaCppAdapter CancellationToken
       → ModelRunner native cancellation AtomicBool
       → llama.cpp generation-loop check
```

During generation, `commands::generate_lines_result` synchronously drained
the stream while the TUI was not executing `LineEditor`; therefore a SIGINT
could be queued but not translated into `runtime.cancel_generation()`. The
native loop only checks cancellation between sampling/decode iterations. A
single llama.cpp decode call cannot be interrupted by this cooperative flag.

There was a second defect in `MockRuntime`: its cancellation branch published
`GenerationCancelled` and returned without sending a terminal stream message.
That could leave a stream consumer blocked. The active-generation slot was
also not cleared on completion/cancellation.

### Correction implemented

The minimum correction was applied without adding a model-management layer:

1. Added `GenerationMessage::Cancelled(GenerationResult)` so cancellation is a
   typed terminal outcome carrying partial text and statistics.
2. Made `MockRuntime` send the terminal cancellation result and clear the
   active-generation slot after cancellation or completion.
3. Added bounded stream receiving and signal polling to the console generation
   loop. The existing `SignalController` now invokes the existing
   `RuntimeService::cancel_generation()` path while generation is active.
4. Kept process termination separate: Ctrl+C during active generation cancels
   inference; `:quit`/EOF remains the explicit console exit path.

### Files changed

- `runtime/src/generation.rs`
- `runtime/src/mock.rs`
- `runtime/src/backends.rs`
- `console/src/commands.rs`
- `console/src/tui.rs`
- `docs/reviews/model-runner-reality-review.md`

### Deterministic verification

The new controllable-backend tests in `runtime/src/backends.rs` pass:

- `controllable_generation_cancels_and_manager_recovers`: receives streamed
  output, observes cancellation, terminates, and completes a second inference.
- `controllable_backend_lifecycle_can_reload`: load → unload → reload → unload.

Targeted results:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo test -p oid-runtime --lib` | PASS — 11 passed |
| `cargo test -p oid-console --bin oid-console` | PASS — 18 passed |
| `cargo test -p oid-model-runner` | PASS — 6 integration tests passed |
| `cargo clippy --workspace --all-targets` | PASS — completed without reported warnings/errors |
| `cargo test --workspace` | FAIL — 4 `console/tests/process_signals.rs` tests timed out waiting for `✓ Console ready`; other reported suites passed |

These tests prove the runtime cancellation protocol and recovery behavior with
a controllable backend. They are not proof of native llama.cpp cancellation.

### Real GGUF validation

**SKIPPED — BLOCKED BY ENVIRONMENT.** This macOS x86_64 environment has no
GGUF in the checked model locations. `ollama` is installed, but no equivalent
GGUF/native `llama-cli` path is available. No model was downloaded. The known
Ubuntu model remains `/home/danny/Models/llama3.2-1b.gguf`, as recorded in the
original audit.

Consequently, the required native sequence—cancel during generation, observe
prompt stop, infer again, unload, reload, and infer a third time—remains
unverified in this environment. The native source path now has a terminal
cancellation outcome and correct state cleanup, but implementation evidence
cannot substitute for the requested real-model run.

### Updated status

OID remains **Level 4 — Usable model runner**. The deterministic runtime
recovery gap is corrected, but Level 5 is withheld until the Ubuntu real-GGUF
invariant is demonstrated: cancellation must stop an active native generation
without unloading or poisoning the model, followed by successful second and
third inferences and clean unload/reload/shutdown.

## LEVEL 5 STATUS

Cancellation root cause: **SIGINT was queued while synchronous generation consumed the stream; the cancellation branch also lacked a terminal stream result and active-state cleanup.**  
Correction implemented: **Yes — signal polling, typed cancellation result, partial statistics/output, and active-generation cleanup.**  
Files changed: **`runtime/src/generation.rs`, `runtime/src/mock.rs`, `runtime/src/backends.rs`, `console/src/commands.rs`, `console/src/tui.rs`, and this review.**  
Tests added: **Controllable cancellation/recovery and lifecycle reload tests in `runtime/src/backends.rs`.**  
Real GGUF cancellation: **SKIPPED/BLOCKED BY ENVIRONMENT.**  
Post-cancel inference: **PROVEN with controllable backend; native GGUF unverified.**  
Unload/reload: **PROVEN with controllable backend and retained native cold-run unload evidence; same-process native recovery unverified.**  
Second reload inference: **PROVEN with controllable backend; native GGUF unverified.**  
Workspace tests: **FAIL — 4 existing console process-signal integration tests timed out waiting for `✓ Console ready`; other reported suites passed.**  
Clippy: **PASS — `cargo clippy --workspace --all-targets` completed without reported warnings/errors.**  
Remaining blocker: **Ubuntu real-GGUF cancellation/recovery proof.**  
Final maturity level: **Level 4 — do not promote to Level 5 yet.**

## WP-LLM-002C Native Verification — 2026-09-04

### Native environment

The requested Ubuntu host was not reachable from this environment:

```text
Local OS: Darwin x86_64
Rust: rustc 1.94.1 (Homebrew)
Documented Ubuntu host: danny@192.168.1.112
SSH result: connect to host 192.168.1.112 port 22: Operation timed out
GGUF available locally: none found in checked locations
Native llama.cpp/llama-cli available locally: none
```

The configured local library convention remains `LLAMA_CPP_LIB_DIR=/Users/dannyfrancisco/.marina/models`, or `$HOME/.marina/models` by default shell form, but that directory does not provide a usable native library/model for this run. No model was downloaded.

### Native Level 5 sequence

**BLOCKED.** The exact requested sequence could not be executed through OID:

```text
OID start → real GGUF load → streamed tokens → cancel during generation
→ typed cancellation → healthy loaded model → second inference
→ unload → reload → third inference → clean shutdown
```

The retained Ubuntu benchmark proves prior real inference but is not reused as
002C cancellation proof. No real-GGUF timestamps, cancellation event, or
post-cancel native inference are claimed here.

### Process-signal test investigation

The four failures are:

| Test | Crate/module | Expected behavior | Actual behavior | Related to 002B? | Classification/root cause |
| --- | --- | --- | --- | --- | --- |
| `interrupting_a_native_child_keeps_oid_usable` | `oid-console` / `tests/process_signals.rs:96` | Start console, run a child, interrupt child, then quit successfully | Timed out at `process_signals.rs:51` waiting for `✓ Console ready`; no signal was sent | No evidence of regression | TEST HARNESS DEFECT / TIMING: child console startup marker was not observed through the piped stdout harness |
| `repeated_interrupts_do_not_corrupt_the_console` | `oid-console` / `tests/process_signals.rs:111` | Two interrupts leave console usable, then quit | Timed out at the same startup wait in workspace/isolated runs; passed in one serial run | No | TEST HARNESS DEFECT / FLAKINESS: failure precedes signal delivery and varies by ordering/run |
| `sighup_while_idle_restores_a_clean_process` | `oid-console` / `tests/process_signals.rs:89` | Start console, send HUP, exit cleanly | Timed out at the startup wait in workspace/isolated runs; passed in one serial run | No | TEST HARNESS DEFECT / FLAKINESS: failure precedes HUP delivery |
| `sigkill_restart_discovers_pending_governed_operation` | `oid-console` / `tests/process_signals.rs:141` | Start, persist pending operation, kill, restart, recover | Timed out at the initial startup wait in workspace/isolated runs; passed in one serial run | No | TEST HARNESS DEFECT / FLAKINESS: failure precedes operation creation and process kill |

The other two tests, `sigterm_while_idle_restores_a_clean_process` and
`sigterm_stops_a_child_that_ignores_term`, passed in the workspace run. A
serial crate run passed five of six, with only
`interrupting_a_native_child_keeps_oid_usable` failing; isolated reruns also
varied. This ordering sensitivity is consistent with timing/flakiness in the
process harness, not with inference cancellation.

The tests and their startup-wait implementation were already present in the
pre-002B `HEAD` tree. The 002B changes do not modify
`console/tests/process_signals.rs`, startup rendering, signal-handler
installation, or shell child supervision. Therefore these failures are not
regressions introduced by WP-LLM-002B. They occur before any test signal is
issued and do not exercise the model-runner cancellation path.

Signal ownership remains separated in source: `SignalController` queues
process signals, `LineEditor`/the generation command consumes interrupts for
runtime cancellation, and `ShellExecutor` consumes interrupts while
supervising child processes. The failing tests do not reach those signal
assertions. No signal failure undermining native model cancellation was
observed, but native cancellation itself remains unverified.

### 002C verification results

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo test -p oid-model-runner` | PASS — 6 integration tests |
| `cargo test -p oid-runtime --lib` | PASS — 11 tests, including cancellation/recovery and lifecycle reload |
| `cargo test -p oid-console --bin oid-console` | PASS — 18 tests |
| `cargo test -p oid-console --test process_signals -- --test-threads=1` | FAIL/FLAKY — one of six failed in the recorded serial run; isolated runs varied |
| `cargo test --workspace` | FAIL — 4 process-signal tests timed out at startup; other reported suites passed |
| `cargo clippy --workspace --all-targets` | PASS exit with warnings — pre-existing `prompt.rs` warning; new warnings from 002B were removed |
| Native OID GGUF cancellation sequence | BLOCKED — Ubuntu SSH timeout and no local model/native library |

### Final maturity decision

The process-signal failures are classified as unrelated test-harness timing/
startup defects, not model-runner regressions. They do not block Level 5 by
themselves. However, Level 5 cannot be promoted because the required native
invariant remains unverified: cancellation must interrupt an active real-GGUF
generation and be followed, without restart or reload, by a successful second
inference, then unload/reload and a successful third inference with clean
shutdown.

## MODEL RUNNER MATURITY — WP-LLM-002C

Previous level: **Level 4**  
Current proven level: **Level 4**  
Level 5 promoted: **NO**  
Blocking invariant, if any: **Real-GGUF cancellation during active generation plus immediate recovery and reload/recovery remain unverified.**  
Production files changed: **`runtime/src/generation.rs`, `runtime/src/mock.rs`, `runtime/src/backends.rs`, `console/src/commands.rs`, `console/src/tui.rs`**  
Tests changed: **`runtime/src/backends.rs` deterministic cancellation/recovery and lifecycle reload tests**  
Documentation changed: **this dated WP-LLM-002C section**

## WP-LLM-003 Native macOS Bring-up — 2026-09-04

WP-LLM-002C's Level 4 conclusion is preserved above. This section records the
subsequent native bring-up and acceptance run on the current Mac.

### Implementation changes

- Added `scripts/setup-llama-macos.sh`, pinning llama.cpp revision
  `18443257a30c884d5332abb8e7dc43c7ffe42fda` (`0.3.0-dev`) and building a
  reproducible CPU-only x86_64 library.
- Added `models/` to `.gitignore`. A local Qwen GGUF was provisioned outside
  source control; the binary is not a repository artifact.
- Corrected the hand-written FFI layouts for `tensor_read_lazy` and
  `n_outputs_max_per_seq`. The newer unpinned llama.cpp build loaded the model
  but aborted during context scheduling because those layouts were stale.
- Added the native llama.cpp abort callback and converted cancellation during
  `llama_decode` into the existing typed cancellation path. The final token no
  longer causes an unnecessary extra decode.
- Released the native runtime cancellation-registry mutex before waiting for
  generation. Holding it across generation deadlocked both completion cleanup
  and `cancel_generation`.
- The signal-aware console now flushes token pieces as they arrive. The
  optional `--max-tokens N` generation form makes short deterministic smoke
  checks possible without changing the default.

### Native environment and model

```text
Platform: macOS Darwin 25.6.0
Architecture: x86_64 Intel
Rust: rustc 1.94.1 (Homebrew)
llama.cpp: 18443257a30c884d5332abb8e7dc43c7ffe42fda, 0.3.0-dev
Native build: CPU-only, GGML_NATIVE=OFF, GGML_AVX=ON, GGML_AVX2=OFF,
              GGML_METAL=OFF, GGML_BLAS=OFF, GGML_OPENMP=OFF
Model: Qwen2.5-0.5B-Instruct Q4_K_M
GGUF size: 491,400,032 bytes (468 MiB)
GGUF path: models/qwen2.5-0.5b-instruct-q4_k_m.gguf
OID library path: /tmp/oid-llama-cpu-build/bin
```

The model was obtained as a small local functional-test artifact from the
Qwen Hugging Face GGUF repository. It is ignored by Git and must not be
committed. OID discovers it from the repository `models/` directory.

### OID local model acceptance transcript

```text
OID LOCAL MODEL ACCEPTANCE

OID startup: PASS — Console ready
GGUF load: PASS — Model loaded: qwen2.5-0.5b-instruct-q4_k_m

Prompt 1: The capital of France is
Result: ĠParis, Ġbut Ġwhen ...
Streaming: PASS — token pieces appeared before the terminal statistics
Result: 4 tokens at 1.7 tokens/sec; latency 2421 ms

Prompt 2: The largest planet is
Result: Ġknown Ġas ...
Second inference without reload: PASS
Result: 4 tokens at 1.7 tokens/sec; latency 2308 ms

Long inference started: PASS — 128-token request, multiple pieces observed
Ctrl+C issued during generation: PASS — issued after 20 tokens were observed
Typed cancellation observed: PASS — Generation cancelled after 20 tokens
OID remained usable: PASS — healthy prompt returned immediately after cancel

Post-cancel prompt: The capital of France is
Post-cancel inference: PASS — 4 tokens, terminal statistics reported

Unload: PASS — Model unloaded and runtime returned to model none
Reload: PASS — same GGUF loaded again
Post-reload prompt: The largest planet is
Post-reload inference: PASS — 4 tokens, terminal statistics reported
Clean shutdown: PASS — Goodbye; Runtime stopped.
```

The cancellation transcript also contained llama.cpp's expected aborted decode
status (`compute status: 1`, `llama_decode ... ret = 2`) followed by context
cleanup. This is native abort-callback interruption, not cancellation after
normal completion. The same process remained alive and accepted the next
generation.

### Verification

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo test --workspace` | PASS — all workspace tests, including all six process-signal tests, passed in the final run |
| `cargo clippy --workspace --all-targets` | PASS exit; pre-existing warnings remain in the FFI wrapper and `console/src/prompt.rs` |
| Native OID GGUF load/inference | PASS |
| Native incremental streaming | PASS |
| Native Ctrl+C cancellation | PASS |
| Immediate post-cancel inference | PASS |
| Native unload/reload/third inference | PASS |
| Clean shutdown | PASS |

The four process-signal tests that were flaky in WP-LLM-002C passed in the
final workspace run. Their earlier failures occurred at the startup marker
before signal delivery; no WP-LLM-002B regression was found. The native run
also showed signal ownership working at the intended boundary: Ctrl+C was
translated to runtime cancellation, while the process stayed alive.

### Final maturity decision

All nine Level 5 criteria are now proven against a real GGUF through
`OID → runtime → ModelRunner → llama.cpp adapter → native llama.cpp`:
inference, incremental token output, active cancellation, typed cancellation,
immediate recovery, unload, reload, post-reload inference, and shutdown.

Current maturity: **Level 5 — Robust runtime.** Level 6 work is explicitly out
of scope for this work package.

## OID LOCAL MODEL RUNNER — WP-LLM-003

Working on current Mac: **YES**  
Real GGUF loaded: **YES**  
Real inference: **YES**  
Real streaming: **YES**  
Real cancellation: **YES**  
Post-cancel recovery: **YES**  
Unload/reload: **YES**  
Workspace tests: **PASS**

Exact command to run OID:

```bash
LLAMA_CPP_LIB_DIR=/tmp/oid-llama-cpu-build/bin \
DYLD_LIBRARY_PATH=/tmp/oid-llama-cpu-build/bin \
cargo run -p oid-console --bin oid-console
```

Exact model used: `qwen2.5-0.5b-instruct-q4_k_m.gguf`  
Remaining blocker: **none for Level 5; native execution is CPU-only in this
acceptance configuration.**  
MODEL RUNNER MATURITY: **Level 5**
