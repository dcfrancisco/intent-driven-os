# Talvik Runtime Release Readiness (Marina Compatibility)

This checklist separates a local, user-scoped Marina release from a remote
deployment. Compilation is not runtime qualification.

Talvik is the provisional primary identity. This file retains its historical
name so existing evidence links remain stable. The `$HOME/.marina` store,
socket location, and environment aliases remain compatibility surfaces; the
old Marina executables are no longer release surfaces.

## Current regression state — 2026-10-17

The current source was rebuilt and executed against the real Qwen GGUF on
macOS x86_64. The previous build accepted raw prompts without applying the
model's embedded `tokenizer.chat_template`, reproducing unrelated output for
`Hello from Marina`. The native adapter now applies the embedded template
before tokenization. The corrected build produced usable streamed text and
passed independent-request, cancellation, unload/reload, and post-reload
inference checks. See the dated evidence in the
[model-runner reality review](reviews/model-runner-reality-review.md).

This correction supersedes the earlier inference evidence for prompt-quality
claims; prior completion/exit-status records remain historical traceability,
not current correctness evidence.

## Local-only release acceptance

- [x] Native release binary loads the pinned local llama.cpp libraries.
- [x] GGUF discovery and model loading use the user-scoped Marina home.
- [x] Loopback HTTP authentication is enabled by default.
- [x] Model discovery, inference, model administration, and cancellation use
      separate bearer scopes.
- [x] Self-cancellation works; cross-principal cancellation requires
      `inference:cancel:any`.
- [x] One active generation is enforced and its permit is released after
      success, cancellation, timeout, and native error paths.
- [x] Request IDs and independent Marina generation IDs remain correlated.
- [x] Conservative memory/context admission rejects unknown or insufficient
      capacity.
- [x] Repeated native success, cancellation, timeout/recovery, and negative
      authorization sequences pass on macOS x86_64.
- [x] Durable OID inference evidence remains append-only, privacy-preserving,
      and recoverable after process restart.
- [x] Native CPU graph diagnostic is classified as an intentional
      cancellation/timeout abort when `llama_decode` returns status 2 with an
      active cancellation flag. Concurrent non-abort native statuses remain
      failures. Upstream error-level logging during abort remains documented.
- [x] Native Linux x86_64 baseline qualified on Ubuntu 26.04 / AMD Phenom II
      X6 with the user-scoped installer payload and relocatable native library
      dependencies.

## Remote deployment acceptance

Remote deployment is not qualified by the current local release. Before
enabling it, all of the following are required:

- [ ] Authenticated private transport or mutually authenticated TLS.
- [ ] Explicit remote authorization and credential rotation policy.
- [ ] No direct unauthenticated LAN/public binding.
- [ ] Request deadline, cancellation, and retry behavior tested across network
      interruption.
- [ ] Resource admission and evidence behavior verified on the target host.
- [ ] OID client compatibility verified against the deployed endpoint.

## OID image readiness

The first bootable OID image is a separate acceptance gate under ADR-0023 and
WP-0088. Image composition assets exist, but the following remain unverified:

- [ ] Real Debian/Ubuntu image build from the documented native Linux host.
- [ ] VM boot with Marina and OID systemd services under separate identities.
- [ ] Missing-model degraded boot and offline startup.
- [ ] Real GGUF inference through OID's governed response path.
- [ ] Governed plan/authorization/execution/verification evidence.
- [ ] Evidence survival across service restart and reboot.
- [ ] Image checksum/manifest and recovery behavior on the target VM.

The image milestone must not be inferred from Marina's qualified local runtime
or from compilation of the image builder.

## Platform qualification

| Target | Build | Native runtime | Status |
| --- | --- | --- | --- |
| macOS x86_64 | Verified | GGUF, HTTP, cancellation, timeout, recovery | Qualified for local-only release with documented upstream abort logging |
| macOS arm64 | No Apple Silicon host available | Not executed | Blocked: requires Apple Silicon runtime host |
| Linux x86_64 | Verified on Ubuntu 26.04 / AMD Phenom II X6 | GGUF, Unix socket, HTTP, auth, admission, cancellation, timeout, recovery, OID evidence | Qualified for the tested legacy CPU baseline; GitHub Actions runner remains pending |
| Windows x86_64 | No Windows host or GitHub Windows runner available | Not executed | Blocked: requires Windows runtime host and native DLL payload |

The qualification script is [`scripts/qualify-marina-http.sh`](../scripts/qualify-marina-http.sh).
It assumes a running daemon and requires `MARINA_URL`, `MARINA_API_KEY`, and
`MARINA_MODEL`; it does not print the API key. Set `QUALIFY_REPORT` to write a
machine-readable `marina.runtime.qualification/v1` JSON report.

For Linux x86_64, use `scripts/setup-llama-linux.sh` and follow
`docs/validation/marina-linux-native.md`. The portable baseline disables
optional x86 SIMD extensions; optimized builds require separate qualification.

## Containerized Linux verification — 2026-10-23

With Docker Desktop running, `scripts/validate-linux.sh` was executed from
the final source revision. The Ubuntu 24.04 image built successfully and the
container completed `cargo fmt --all -- --check` and `cargo build --workspace`.
The script then stopped at strict Clippy with 94 diagnostics under Rust
1.99.1, before its test phase. The diagnostics are in the existing workspace
lint baseline (not a Talvik runtime failure); the host comparison remains the
authoritative source/baseline comparison recorded below. This is therefore a
successful container build check, but not a passing end-to-end Docker
validation. Native Linux host qualification remains separate and is not
claimed from this container run.

## CPU-first closeout audit — 2026-10-17

Final native inputs were macOS x86_64, the Qwen GGUF
`qwen2.5-0.5b-instruct-q4_k_m.gguf` (SHA-256
`74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`), and
`$HOME/.marina/lib/libllama.0.3.0.dylib`. An uninterrupted final-binary CLI
run produced relevant decoded text (`Paris<br>France`) and
`DONE\tcompleted\t5`, with no control-token or tokenizer-byte leakage. The
embedded-template native test and full workspace test suite passed.

Strict workspace Clippy was compared with baseline revision `6034903`: both
reported 92 diagnostics with identical diagnostic lines. These are
pre-existing workspace findings; no new final-runner finding was introduced.

This was initially **NO-GO** because the diagnostic harness omitted the
required newline and used an insufficient timeout. That finding is superseded
by the corrected harness and lifecycle evidence recorded below.

### Resolved lifecycle findings

- `MARINA-F-001`: the stalled probe omitted the newline required by Marina's
  line protocol; the checked-in harness now constructs frames explicitly.
- `MARINA-F-002`: Marina released the active-generation permit only after
  sending the terminal event; terminal ordering is corrected and verified.
- `MARINA-F-003`: four console process-signal tests remain open as a separate
  startup-marker environment issue under WP-0056. This does not block the
  native Marina lifecycle gate, but requires a stable process-test rerun.

The corrected final-binary lifecycle run passed independent prompts, streaming,
real cancellation, post-cancel recovery, unload/reload, post-reload inference,
status, and shutdown. The CPU-first Marina local-runner gate is **GO**.

## Talvik productization verification — 2026-10-17

The primary `talvik` release binary was built against the same pinned native
libraries and real Qwen GGUF. The first productization lifecycle rerun exposed
`TALVIK-F-001`: the llama.cpp adapter published its terminal event before
clearing its private generation-admission flag. This was distinct from the
previously corrected runtime-service slot. The adapter now restores `Ready`
and clears its flag before publishing completed, cancelled, or failed events.

The corrected lifecycle run passed:

- independent prompts without context leakage: `Paris` and `4`;
- incremental first-token delivery and `DONE\tcompleted`;
- cancellation after one token with `DONE\tcancelled\t1`;
- immediate post-cancellation recovery with `Jupiter`;
- unload, empty loaded-model status, fallback-driven reload, and post-reload
  inference with `READY`;
- explicit fallback from `missing-model` to the Qwen model;
- uninterrupted direct CLI inference with `Tokyo` and `DONE completed`;
- process shutdown after acceptance, with the socket no longer accepting
  requests.

Routing remained disabled by default and required both
`TALVIK_ROUTING_ENABLED=1` and an ordered `TALVIK_ROUTE_MODELS` list. Fallback
was not permitted after output began.

Strict workspace Clippy still reports the same 92 pre-existing diagnostics as
the recorded baseline. A strict no-dependency comparison of the compatibility
`marina` binary and the new `talvik` binary reports the same 17 diagnostic
headings; there are no Talvik-specific strict findings. Four unrelated
`oid-console` process-signal tests continue to reproduce `MARINA-F-003` while
two SIGTERM cases pass. That finding remains tracked under WP-0056 and does not
invalidate the standalone runtime lifecycle evidence.

The CPU-first standalone Talvik runtime gate is **GO** on the qualified macOS
x86_64 host. Cross-platform publication, public-name clearance, capability-
aware multi-backend routing, and remote-provider qualification remain separate
release gates.

Final release artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| `target/release/talvik` | `0f9ffcd0c5177a0a46ccb773d3578a5e5f62191b9aac2bbc61f5bbb778258a6c` |
| `target/release/marina` | `0722a82dfd7ba63b2d9ba1f06d521ee4554721ca6402ab7f95fd867f137dafb9` |
| `target/release/marinactl` | `fdbc31f2d5b41124517b90514d676d1e500ccd927468a93e180e603eeb108168` |
| Qwen acceptance GGUF | `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db` |

## Talvik executable split verification — 2026-10-22

The release surface now has two explicit processes: `talvikd` is the daemon
and `talvik` is the client. The former `marina` and `marinactl` Cargo targets,
installer outputs, release artifacts, systemd unit, and active qualification
commands were removed. This does not migrate or rename persisted data:
`$HOME/.marina`, `$HOME/.marina/marina.sock`, and `MARINA_*` aliases remain
compatible, with `TALVIK_*` overrides taking precedence.

Verification completed on macOS x86_64:

- `cargo test --workspace`: passed;
- debug daemon/client socket smoke: passed outside the restricted sandbox;
- release `talvikd`/`talvik` socket smoke: `Healthy`, `llama.cpp`, and model
  list completion through an explicitly isolated temporary home;
- obsolete `cargo build -p oid-console --bin marina`: rejected, with only
  `talvikd`, `talvik`, `oid-console`, and `oid-marina-chat` exposed;
- strict Clippy comparison: baseline `main` and this revision each report 90
  actual pre-existing findings (92 `error:` lines including Cargo summaries),
  so the executable split introduces zero net findings;
- shell and packaging script syntax checks: passed.

Fresh release artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| `target/release/talvikd` | `c113ca23cc8feade26c6fc68fb3b1dc1fb72268e0bf3a9aab44dfb74f0996e0e` |
| `target/release/talvik` | `52bcd6f3d61c176184d83b83e0f4bcb5e117fa43daa807f3171f2e7881b54f13` |

## Talvik M1 feature-branch verification — 2026-10-23

This entry supersedes the stale single-model wording above for the current
feature branch. Fresh release binaries were built with the pinned
`$HOME/.marina/lib` llama.cpp library and tested with the Qwen
`qwen2.5-0.5b-instruct-q4_k_m.gguf` and DeepSeek `deepseek-coder-1.3b.gguf`
artifacts, without changing `$HOME/.marina`.

- Multi-model residency: PASS. Both real GGUFs loaded concurrently; each was
  addressed by model ID; unloading Qwen left DeepSeek runnable; unloading the
  final model returned an empty resident set.
- Uninterrupted and isolated generation: PASS. Qwen returned `Paris` and `4`
  for independent prompts; the lifecycle harness returned `Paris`, `4`, and
  `Jupiter` after cancellation recovery with `DONE completed`.
- Streaming/cancellation/recovery: PASS. First-token delivery occurred before
  terminal events; cancellation returned `DONE cancelled`; the next request
  completed immediately afterward.
- Chat templates: PASS. Multi-turn HTTP messages were passed as distinct role
  records to the embedded GGUF template; raw prompt fallback remains limited
  to models without a template, and template application errors remain errors.
- Embeddings: PASS. `/v1/embeddings` returned a real non-empty 896-value vector from the
  native model. Tool requests are explicitly rejected with a capability error
  because llama.cpp tool execution is not implemented in this milestone.
- Workspace tests: PASS with `DYLD_LIBRARY_PATH=$HOME/.marina/lib`; a native
  debug test launch without that path fails at dynamic linking before test
  execution. Installed release binaries use the managed-prefix rpath.
- Strict Clippy: current default-feature run reports 92 findings, matching
  baseline revision `6034903`; the findings are pre-existing. Native
  all-features linting also retains existing FFI lint findings.

Fresh M1 release artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| `target/release/talvik` | `200008190efff0c6d19ff09350dc9527268dfc931d287c8b7137532187564a95` |
| `target/release/talvikd` | `ad3464490db08629cdbdd56f0beb112f96160902edd40b340e690e75f848cb28` |

The CPU-first Talvik M1 implementation gate is **GO** for the qualified macOS
x86_64 environment, with cross-platform publication and tool execution still
separate limitations.

The previously recorded real-GGUF lifecycle qualification remains applicable
because the runtime, transport protocol, and model state paths are unchanged;
this revision changes executable entry points and product packaging only.
