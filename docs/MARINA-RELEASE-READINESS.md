# Marina Runtime Release Readiness

This checklist separates a local, user-scoped Marina release from a remote
deployment. Compilation is not runtime qualification.

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
