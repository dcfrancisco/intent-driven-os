# WP-0056: Process and Signal Reliability

## Status

Implemented for the tested process harness — macOS and Ubuntu 24.04 Docker
validation complete; real-TTY/host validation remains. The four previously
reported startup-marker failures were reproduced only in the earlier loaded
workspace run. A fresh standalone run passed all six tests serially and with
the default test-thread setting. The required qualification command is the
serial form below because these tests start and signal real child processes.

## Purpose

Prove that OID remains a usable terminal and preserves governed-operation
recovery semantics when users interrupt commands, the runtime shuts down, or
the process terminates abnormally.

## Scope

- Ctrl+C and repeated Ctrl+C behavior.
- Ctrl+D and clean end-of-input handling.
- SIGINT, SIGTERM, and SIGHUP handling.
- Child-process termination and cleanup.
- OID graceful shutdown and durable-state flushing.
- Terminal restoration after interruption or shutdown.
- Interrupted governed-operation state and recovery discovery.
- Recovery after SIGKILL, which cannot be handled or made graceful.

## Invariants

- An interrupted native command leaves OID alive and the terminal usable.
- Graceful shutdown flushes durable state, handles children, and restores the
  terminal.
- An interrupted governed operation is never silently marked successful.
- Restart discovers interrupted or uncertain operations.
- Inspect, resume, and rollback remain available after restart when their
  safety preconditions are satisfied.
- SIGKILL is treated as an interruption/recovery test, not as a graceful
  shutdown path.

## Required scenarios

- Native command interrupted with Ctrl+C.
- Repeated Ctrl+C while a child is active.
- SIGTERM while a child is running.
- SIGHUP during an active console session.
- A child that ignores SIGTERM.
- SIGTERM during evidence persistence.
- SIGKILL immediately after execution and before verification.
- SIGKILL during rollback.
- Clean Ctrl+D and normal OID shutdown.

## Acceptance criteria

- Signal and child-process tests pass on the supported Linux environment.
- The parent console remains usable after native-child interruption.
- No orphaned child processes remain after supported shutdown paths.
- Terminal modes and standard streams are restored after interruption.
- Journal state distinguishes completed, failed, interrupted, and uncertain
  execution outcomes without claiming success prematurely.
- Restart exposes every interrupted or uncertain operation with safe,
  user-directed recovery options.
- Durable state is flushed before graceful shutdown reports completion.
- SIGKILL recovery is validated by killing and restarting a real process.

## Out of scope

- New skills or dynamic capabilities.
- Model integration or intent interpretation changes.
- Inline approval UX.
- Desktop UI, phone, voice, or plugin expansion.
- Treating SIGKILL as catchable or graceful.

## Dependencies

- WP-0050 Recovery and Resume CLI Boundary.
- WP-0055 Terminal Identity and Shell Passthrough.
- `v0.1.0-alpha.1` governed-operation baseline.

## Exit condition

When this package passes, proceed directly to State and Corruption Resilience.
Do not add product capabilities between the two stabilization packages.

## Qualification disposition — 2026-10-23

```text
cargo test -p oid-console --test process_signals -- --test-threads=1 --nocapture
```

Result: **6 passed**. A standalone default-thread rerun also returned **6
passed**. The earlier four failures stopped in
`wait_for_text("✓ Console ready")`, before signal delivery or governed-operation
creation; they did not produce a runtime, transport, or child-cleanup failure.
No implementation fix was required. Keep the serial command as the
deterministic gate and retain real-host TTY qualification as a separate
platform obligation.
