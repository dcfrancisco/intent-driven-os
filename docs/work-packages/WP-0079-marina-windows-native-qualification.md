# WP-0079: Marina Windows x86_64 Native Qualification

## Purpose

Complete the Windows x86_64 qualification obligations transferred from WP-0077
using native Windows execution.

## Scope

- Build Marina and the pinned llama.cpp revision with the Windows toolchain.
- Package and load the matching native DLL payload.
- Run real GGUF inference and the qualification sequence.
- Verify loopback TCP defaults, Windows user-scoped configuration, token
  authorization, admission, cancellation, timeout, recovery, and OID evidence.
- Record DLL ABI, runtime-library, CPU instruction-set, filesystem, and TCP
  behavior in a machine-readable report.

## Acceptance criteria

- Native Windows execution completes discovery and real GGUF inference.
- Cancellation, timeout, native abort classification, and recovery pass.
- Loopback TCP remains local by default and persistent YAML/environment
  configuration behaves as documented.
- The release uses user-scoped installation paths and matching DLL versions.

## Dependencies

WP-0077, ADR-0021, WP-0065, and a Windows x86_64 host or GitHub Windows
runner with the native DLL build environment.

## Status

Blocked: no Windows runtime host or runner is currently available.
