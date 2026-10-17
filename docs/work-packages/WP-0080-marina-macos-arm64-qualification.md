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

## Status

Blocked: no Apple Silicon host is currently available.
