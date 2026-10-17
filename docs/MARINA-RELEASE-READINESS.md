# Marina Runtime Release Readiness

This checklist separates a local, user-scoped Marina release from a remote
deployment. Compilation is not runtime qualification.

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
It assumes a running daemon and requires `MARINA_URL`, `MARINA_TOKEN`, and
`MARINA_MODEL`; it does not print the token. Set `QUALIFY_REPORT` to write a
machine-readable `marina.runtime.qualification/v1` JSON report.

For Linux x86_64, use `scripts/setup-llama-linux.sh` and follow
`docs/validation/marina-linux-native.md`. The portable baseline disables
optional x86 SIMD extensions; optimized builds require separate qualification.

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
