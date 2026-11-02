# ADR-0020 implementation traceability — 2026-11-02

Baseline revision: `c6a224646d2de2bbb37515af8d9c74476af1586b` on `main`.
Assessment host: macOS x86_64. This report distinguishes source implementation,
tests, qualification, and blocked external gates.

## Decision conflict discovered

The implementation request calls ADR-0020 previously accepted. The repository
source of truth currently labels ADR-0020 **Proposed**, and WP-0076 **Proposed**.
ADR-0021 is also **Proposed**, while WP-0077 remains **In progress**. The
checkpoint-8 review permits ADR-0020 Phase 1 planning and identifies the native
API/admission governance gate as locally satisfied, but says training execution
has not started. This slice does not change any decision status and does not
start a training worker. The governance owner must resolve the status mismatch.

The request's job-state vocabulary begins with `CREATED`; ADR-0020 and WP-0084
begin with `PENDING`. The implementation retains `PENDING` and documents the
mapping rather than silently overriding the repository decision.

## Phase 0 baseline

Repository inspection found:

- real GGUF inference, streaming, cancellation, model residency, memory/context
  admission, persistent inference registry metadata, bearer authentication,
  native HTTP/OpenAI and bounded Ollama-compatible edges;
- no model-development subsystem, dataset registry, training job store,
  executable training backend, adapter lifecycle, evaluation, promotion,
  rollback, export, conversion, or quantization pipeline;
- model creation/copy/delete and Ollama registry lifecycle explicitly reported
  unsupported in the capability matrix and gap review;
- training work decomposed by ADR-0020 into WP-0082 through WP-0087, all
  originally planned, with WP-0082/WP-0083 the first dependency-ready,
  hardware-independent work;
- no missing ADR-0020-related WP files. All ADRs named by ADR-0020 and WP-0076
  through WP-0087 exist. The requested "Talvik/Ollama parity and gap reviews"
  are present in `docs/reviews/TALVIK-OLLAMA-RUNNER-GAP-REVIEW.md` and the
  compatibility matrix/roadmap.

The pre-change `cargo test --workspace` compiled through `oid-runtime` but did
not reach test output after several minutes and was interrupted. It is recorded
as **BLOCKED/INCONCLUSIVE**, not passed. Narrow targeted checks are the evidence
for the first implementation slice.

## Traceability matrix

| ADR/request capability | Baseline | Current slice | Evidence/status | Next dependency |
| --- | --- | --- | --- | --- |
| Backend-neutral training contracts | Not started | Method, capability, device, configuration and state-transition value contracts | TESTED by targeted unit suite | Executable process SPI and capability probe, WP-0082 |
| Immutable dataset registration | Not started | Content-addressed registry and restart verification | TESTED by targeted unit suite | Authenticated composition and evidence, WP-0083 |
| JSONL SFT validation | Not started | Prompt/completion and conversational schemas, partitions, UTF-8, bounds, duplicates, full diagnostics | TESTED by targeted unit suite | Sensitive-content policy and API contract |
| Persistent configuration / derived models | Unsupported | Unchanged | NOT STARTED | Model registry/version design; separate bounded slice |
| Persistent external knowledge | No model-development contract | Unchanged | NOT STARTED | Separate storage/retrieval ADR or existing platform integration decision |
| Durable training jobs and restart reconciliation | Not started | State-transition rules only | Contract-only / UNVERIFIED | WP-0084 |
| Worker isolation and resource admission | Inference admission only | No training worker | NOT STARTED | WP-0084 |
| Real CPU training | Not started | No trainer | BLOCKED by accepted contracts/lifecycle, not hardware | WP-0085 |
| Real LoRA | Not started | Declared distinct method only | NOT STARTED; must not be advertised | Executable PEFT backend and bounded model qualification |
| QLoRA | Not started | Declared separate capability and explicit rejection | NOT STARTED / hardware-framework dependent | Separate accelerator qualification |
| Adapters and inference | Not started | Unchanged | NOT STARTED | WP-0087 plus actual llama.cpp adapter compatibility proof |
| Artifact lineage/checkpoints | Inference model checksum only | Dataset lineage only | PARTIAL | WP-0087 |
| Evaluation/base comparison | Not started | Unchanged | NOT STARTED | WP-0086 |
| Promotion/deploy/rollback | Not started | Unchanged | NOT STARTED | WP-0086 and governed/local authorization |
| Export/conversion/quantization | Not started | Unchanged | NOT STARTED | Actual supported toolchain and load/smoke gates |
| Management API/CLI | Proposed routes only | No new routes/commands advertised | NOT STARTED | Reconcile with native API/auth scopes |
| Cross-platform training qualification | None | Portable Rust contracts only | UNVERIFIED outside host | Native hosts and trainer environments |

## First vertical slice

The first dependency-ready slice implements WP-0082/WP-0083 foundations in
`runtime/src/training.rs`. It does not touch inference serving or add Python/ML
dependencies. The dataset registry copies only explicitly selected sources,
uses bounded validation and content addressing, rejects traversal identifiers,
verifies the copy before publication, persists immutable metadata, and checks
the artifact path and digest on restart.

The associated contract is
`docs/TALVIK-MODEL-DEVELOPMENT-CONTRACT.md`. WP-0082 and WP-0083 are marked
**In progress**, with their missing production and qualification gates listed.

Targeted verification on the assessment host passed all seven new tests:

```text
cargo test -p oid-runtime training::tests -- --nocapture
test result: ok. 7 passed; 0 failed; 0 ignored; 51 filtered out
```

The native-enabled test binary required 5 minutes 30 seconds to compile and
link; the tests themselves completed in 0.22 seconds.

Post-change runtime verification then passed:

```text
cargo fmt --all -- --check
cargo clippy -p oid-runtime --all-targets -- -D warnings
cargo test -p oid-runtime
test result: ok. 58 passed; 0 failed; 0 ignored
doc-tests: 0 passed; 0 failed
cargo test --workspace
result: passed; native real-GGUF qualification fixtures remained explicitly ignored
```

The runtime suite was run outside the filesystem/network sandbox because its
existing interrupted-download test binds a loopback-only temporary HTTP server.
The sandboxed attempt produced 57 passes and only that bind-permission failure;
the permitted rerun passed all 58 tests.

## Known limitations and blockers

- The new registry is not composed into `talvikd`; no public operation is
  available, so authentication/audit integration remains open.
- Registry replacement and restart behavior are tested only on the current
  host; Windows atomic-replacement semantics require native qualification.
- No sensitive-content classifier is selected. Validation does not claim one.
- No training backend is installed, invoked, mocked as successful, or
  advertised.
- The host has no qualifying CUDA/ROCm training environment. QLoRA remains
  unqualified even after future LoRA work.
- Existing WP-0077 cross-platform and remote transport gates remain open and
  are not closed by dataset tests.

## Next bounded phase

Complete WP-0082's executable training-backend process SPI and WP-0084's
durable job/reconciliation boundary without invoking a real trainer: explicit
capability probing, stable job IDs, persisted transitions, cancellation,
crash-to-non-success reconciliation, resource requests, and a worker process
contract. Only after that contract and isolation slice is reviewed should
WP-0085 introduce a real bounded CPU trainer.
