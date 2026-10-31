# WP-0111: Working local runner stabilization and RC gates

Date: 2026-10-31
Status: Local stabilization preview; archive qualification is recorded per build. No RC claim.
Milestone: M2.1, a usable standalone local model runner.
Related: ADR-0025, ADR-0026, ADR-0031, ADR-0032; WP-0065, WP-0091,
WP-0092, WP-0098, WP-0105, WP-0108, WP-0109.

## Product outcome

Install Talvik without a development checkout, start one managed daemon, select
an appropriate model, and use a standard local API or an IDE. Routine use must
not require manual shell-variable repair or changes to an IDE extension.
Talvik is the model provider; the coding assistant owns workspace tools.

## Stabilization changes

- Reserve HTTP and IPC listeners before runtime/model startup. Do not unlink a
  live Unix socket; allow stale socket recovery and reject duplicate listeners.
- Serialize generation admission and protect enqueue/active transitions. Real
  Copilot utility and Agent requests must queue instead of racing for a slot.
- Keep standard OpenAI streaming, including requested usage-only chunks. The
  installed Copilot parser accepts these; disabling usage was a false lead.
- Supply Qwen thinking prefill explicitly because the C chat-template API
  chooses built-in formatters and does not evaluate arbitrary Jinja kwargs.
- Remove split terminal markers without leaking their leading angle bracket.
- Include required tool argument names/types in the native tool instructions.
- Provide explicit daemon context/output defaults for IDEs that omit request
  options, while retaining request-level overrides and memory admission.
- Install the native vision library with the Unix payload. Verify macOS
  archive/source layouts, service startup, quoted paths, upgrades, and retained
  user models during uninstall.

## Executable acceptance gates

| Gate | Evidence required | Current status |
|---|---|---|
| Native CPU generation | Real GGUF Chat and coding responses | Passed on local Intel macOS |
| Vision | Correct color from an inline image, without leaked thinking tags | Passed on local Intel macOS |
| Standard streaming | Content/tool deltas, finish reason, requested usage, DONE | Passed in API checks |
| Tool loop | Required file-path argument, correlated tool result, final answer | Passed in API checks |
| Singleton lifecycle | Second launch fails; one port owner; stale socket restart | Passed locally and in regression tests |
| Concurrent scheduling | Synchronized simultaneous requests and real utility/main calls | 64-request regression and real Ask utility/main passed |
| Cancellation | Native long request aborts and next request runs | Passed locally |
| VS Code Ask | Actual extension log records successful local request | Passed; about 54 seconds on current CPU build |
| VS Code Agent | Read a workspace file through an actual IDE tool and answer | Pending; real prompt contains about 22K tokens |
| VS Code Plan | Actual planning request, with truthful tool/context use | Pending |
| Prompt performance | Same prompt and correctness before/after optimization | Correct PERF_OK at 769 tokens: 33.9s to 25.4s prompt processing (1.33x), local CPU only |
| Installer contract | Native vision payload, path escaping, source/archive, preservation | Five executable installer tests, including incomplete-payload rejection and host-library isolation |
| Relocatable release install | Real archive, no developer library paths, status, uninstall | Passed locally; smoke harness clears host runner configuration and uses isolated state |
| One state directory | CLI and HTTP use the same daemon, registry, and model pairings | Passed: canonical registry/socket migration and CLI status use the same daemon |
| Workspace validation | Formatting and all workspace tests | Passed locally; see the build qualification report for totals |
| Strict Clippy | Same strict command as release CI | Passed locally after ownership/assertion cleanup; final combined-build results recorded in the qualification report |
| Release identity | Clean source commit matches CLI, daemon, and HTTP identity | Record against the final archive in the build qualification report |

## RC promotion gates

Do not tag or advertise an RC until the local gates above are recorded against
one clean build and the following release work is qualified:

- Clean-machine installation and upgrade on each advertised OS/architecture.
- Complete native library payloads, including vision dependencies; no hidden
  dependency on a developer checkout or global library search path.
- OpenAI/Ollama compatibility inventory: supported routes have client evidence;
  unavailable operations return explicit errors rather than success stubs.
- Native disconnect/cancellation, queue overflow/deadline, crash/restart, and
  memory-pressure recovery checks, including continuous client use.
- Reproducible native revision/ABI and checksummed, versioned release artifacts.
- Green required formatting, tests, and lint checks; baseline failures remain
  release blockers rather than being relabeled as passes.
- Measured performance and context defaults per supported hardware profile.
  The current Intel Iris Vulkan path generated incorrect output and is not a
  qualified accelerated profile. CPU correctness is the baseline.

Cross-platform or GPU claims must not be inferred from a local CPU pass. This
work package is a release gate, not a claim of parity with every other runner.

## Local qualification, 2026-10-31

All five native API checks passed again with the Apple BLAS CPU payload:
Chat, Coder, red-image vision, streamed required tool arguments, and the correlated
tool-result answer. No trailing `<` or empty thinking wrapper appeared. These
checks are available in `scripts/test-talvik-native-api.py` and do not execute
workspace tools. Real VS Code Ask passed separately; Agent and Plan remain
unqualified because of large cold prompts and current CPU latency.

`TMPDIR=/tmp cargo test --workspace` passed with native linkage. The process
signal test had been waiting for a removed startup banner; its readiness marker
now matches the current console. The Unix singleton fixture accounts for
concurrent shell-test children retaining inherited descriptors until exec.

The local host uses Rust 1.99 while release CI specifies 1.82. Strict Clippy
still reports HTTP helper ownership/size/argument lints and some test assertion
lints. The preview does not waive these as RC promotion requirements. The
release workflow, including other OS/architectures, has not been run remotely.

## API recovery stabilization follow-up

A live pre-token disconnect reproduced an occupied generation after twelve
seconds. HTTP waiting now polls disconnection while awaiting model output,
bounds socket writes and cancels/relinquishes ownership on every transport exit.
The same guard covers the Ollama generation edge. Duplicate request IDs cannot
overwrite another caller’s ownership. Invalid prompt/context errors retain their
type through the backend boundary and return 400 rather than 500; unknown
models return 404 instead of a generic lifecycle/admission response.

`scripts/test-talvik-api-recovery.py` supplies fourteen live recovery cases. The
first native candidate passed all fourteen; final-build requalification is
recorded separately. Official Python SDK discovery and all six text generation
shapes passed (Chat Completions, legacy Completions and Responses, streaming and
non-streaming). See `docs/TALVIK-API-CONTRACT.md` for the bounded surface and
transport/deadline limitations. These results do not waive IDE or platform gates.
