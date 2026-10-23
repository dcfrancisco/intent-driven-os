# WP-0058: Verified Model Cache and Admission

## Purpose

Make model artifacts trustworthy and prevent requests from starting when
context, memory, device, policy, or concurrency constraints cannot be met.

## Scope

Manifest schema, SHA256 verification, cache layout and atomic writes, trust
states, model variant selection, memory/context reservations, and deterministic
admission failures. The first explicit local/HTTP(S) GGUF pull path is now
implemented by `ModelAcquirer`; policy, resumable transfers, and richer
manifests, resumable remote transfers, and progress callbacks remain in this
work package.

## Acceptance criteria

- Unverified or corrupt artifacts cannot load.
- Admission explains each rejection and releases reservations on failure or
  cancellation.
- Model load admission uses a preflight estimate and validates the actual
  backend-reported allocation before publishing `Loaded`; rejected loads return
  to `Registered` without disturbing existing residents.
- Restart preserves registry/cache state or fails closed without corruption.
- Multiple resident models remain independently addressable and admission
  accounts for their aggregate memory.

## 2026-10-23 memory-admission correction

Finding `TALVIK-F-011` showed that generation admission was fail-closed but
model load admission was missing, allowing a model to become `Loaded` before a
default-context request was known to be impossible. The implementation now
preflights the registry estimate and rechecks the backend-reported size after
native load, unloading the candidate and restoring `Registered` on rejection.
The fixed KV/compute estimate is still conservative rather than a direct
llama.cpp allocation measurement; architecture-aware accounting remains future
work under this package.

## Automatic recovery implementation — 2026-10-23

WP-0058 now includes the first bounded pressure-recovery slice. Inference checks
the requested resident model, otherwise attempts normal load/admission, evicts
the least-recently-used idle unpinned model when necessary, refreshes production
hardware availability, and retries within a fixed recovery budget. Registered
metadata is retained after eviction. A bounded FIFO queue waits behind the
existing one-active-generation limit; queued cancellation and admission deadlines
are terminal and do not retry indefinitely. Policy is configurable with
`TALVIK_MAX_RESIDENT_MODELS`, `TALVIK_MEMORY_BUDGET_BYTES`,
`TALVIK_SAFETY_MARGIN_BYTES`, `TALVIK_AUTOMATIC_EVICTION`,
`TALVIK_RECOVERY_BUDGET`, `TALVIK_ADMISSION_TIMEOUT_SECONDS`,
`TALVIK_QUEUE_CAPACITY`, `TALVIK_IDLE_RETENTION_SECONDS`, and
`TALVIK_PINNED_MODELS`.

Acceptance evidence: `TALVIK-F-012` controlled admission/recovery and queue tests
pass; native llama.cpp pressure recovery remains not verified for the unavailable
DeepSeek-R1-Distill-Qwen-1.5B artifact. The implementation does not claim direct
measurement of llama.cpp compute/KV allocations.

## Dependencies

WP-0024, WP-0025, WP-0029; ADR-0006, ADR-0007, ADR-0011.
