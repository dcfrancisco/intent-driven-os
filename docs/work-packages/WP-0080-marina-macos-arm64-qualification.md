# WP-0080: Marina macOS arm64 Native Qualification

## Purpose

Verify that the existing macOS Marina release behavior carries to Apple
Silicon without treating Intel compilation as evidence.

## Scope

- Build the pinned llama.cpp revision for arm64 with the documented CPU or
  Metal configuration.
- Run real GGUF inference and the qualification sequence on Apple Silicon.
- Verify dynamic-library architecture, native instruction selection, model
  discovery, Unix socket/HTTP behavior, admission, cancellation, timeout,
  recovery, and OID evidence.
- Produce a machine-readable qualification report.

## Acceptance criteria

- The arm64 binary and native libraries load without translation requirements.
- Real inference and repeated recovery tests pass.
- Any Metal/CPU abort diagnostic is separately classified and documented.

## Dependencies

WP-0077, ADR-0021, WP-0065, and an Apple Silicon runtime host.

## Reproducible procedure

On an Apple Silicon host, run `scripts/setup-llama-macos.sh`, build the
release binaries with `LLAMA_CPP_LIB_DIR="$HOME/.marina/lib"` and
`LLAMA_CPP_REQUIRED=1`, then execute the native HTTP lifecycle qualification
and `scripts/qualify-openai-client.py` against a real GGUF. Record `uname -a`,
`file` output for both binaries and native libraries, model SHA-256, and the
complete lifecycle report before marking this package qualified.

## Status

Blocked: no Apple Silicon host is currently available.
