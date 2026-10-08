# WP-0078: Marina Linux x86_64 Native Qualification

## Purpose

Complete the Linux x86_64 qualification obligations transferred from WP-0077
without changing Marina runtime architecture.

## Scope

- Build Marina and the pinned llama.cpp revision against Linux native libraries.
- Run real GGUF inference on a supported Linux distribution and CPU.
- Exercise discovery, authentication/scopes, context and memory admission,
  cancellation, timeout, native abort classification, and recovery.
- Verify Unix-socket defaults, configurable loopback HTTP, YAML precedence,
  generation IDs, OID evidence, and installer/library loading.
- Produce a `marina.runtime.qualification/v1` JSON report and update the matrix.

## Acceptance criteria

- No Python bridge is used in the inference path.
- Native model loading and repeated inference pass on the target machine.
- Cancellation/timeout release permits and recover on the same daemon.
- Native abort diagnostics are separated from genuine native failures.
- Linux-specific linker, ABI, CPU instruction-set, filesystem, and socket
  behavior are recorded.

## Dependencies

WP-0077, ADR-0021, the pinned llama.cpp library build, and a Linux x86_64
runtime host or equivalent native VM/container.

The preferred native target for this checkpoint is the Phenom II X6 host at
`192.168.1.112`, accessed through the approved SSH workflow. GitHub Actions
Ubuntu x86_64 qualification is a separate repeatable baseline and does not
substitute for Phenom execution. Docker is a reproducible build/test option,
but Docker-only output remains unverified for native-host qualification.

## Reproducible procedure

The native CPU backend build is provided by
`scripts/setup-llama-linux.sh`. The qualification procedure is documented in
`docs/validation/marina-linux-native.md` and reuses
`scripts/qualify-marina-http.sh` for machine-readable HTTP lifecycle evidence.
The build is intentionally conservative and disables AVX-family extensions in
the baseline; optimized CPU builds require separate evidence.

## Status

**Qualified for the tested native Linux baseline:** Ubuntu 26.04 x86_64 on an
AMD Phenom II X6 1055T. The GitHub Actions Ubuntu baseline is defined but has
not been executed from this workspace; it remains unverified and does not
replace the Phenom evidence.

## Current verification disposition

### Passed

- Native Linux qualification procedure and conservative CPU build script are
  present and syntax-validated.
- Portable workspace tests pass on the current macOS host.
- The report is valid `marina.runtime.qualification/v1` JSON.
- Native Phenom build of pinned llama.cpp and Marina completed.
- Real GGUF discovery, loading, inference, detokenization, Unix socket status,
  loopback HTTP, bearer authentication, scope denial, context rejection,
  cancellation, timeout, same-daemon recovery, unique generation IDs, and OID
  HTTP inference passed.
- Interactive `oid-console` in `marina-http` mode persisted linked started and
  completed inference evidence with metrics after the real request.
- User-scoped installer payload resolved all native libraries from its own
  `lib/` directory without `LD_LIBRARY_PATH` after the `$ORIGIN` RPATH fix.
- Native SIGTERM shutdown completed successfully after the qualification run.

### Failed

- None observed in the available checks.

### Blocked

- None for the tested Phenom Linux baseline.

### Unverified

- Other Linux distributions, CPU families, and older x86_64 variants beyond
  the tested Phenom II CPU remain unverified.
- GitHub Actions Ubuntu runner execution and broader Linux distribution
  qualification remain unverified.

## Checkpoint 10 execution paths

- `.github/workflows/marina-linux-qualification.yml` runs native Ubuntu
  qualification with a caller-supplied public GGUF URL and checksum. It does
  not upload models to GitHub and does not print credentials.
- `scripts/qualify-marina-linux.sh` records Linux host metadata, CPU flags,
  memory, tool versions, Unix-socket status, and the existing HTTP lifecycle
  results in `marina.runtime.qualification/v1` JSON.
- The Phenom run remains the required evidence for the legacy CPU target.
