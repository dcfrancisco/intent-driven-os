# WP-0077 Checkpoint 8 Review

## Decision

WP-0077 SHALL remain **In Progress**. The macOS x86_64 local-runtime slice is
qualified, but the original WP still contains cross-platform compatibility
requirements that were not executed. Those obligations are transferred to
WP-0078 through WP-0081 without being declared satisfied.

ADR-0021 remains the governing transport, authorization, lifecycle, and
admission decision. ADR-0020 Phase 1 planning may begin because the native API
and local admission foundation are usable, but training execution remains
separate from production inference and is not started by this review.

## Acceptance disposition

| Original WP-0077 criterion | Disposition | Evidence or follow-up |
| --- | --- | --- |
| Versioned API without Python bridge | Satisfied locally | Native Rust HTTP and live GGUF inference on macOS x86_64 |
| Linux/macOS Unix-socket compatibility | Partially satisfied | macOS x86_64 verified; Linux transferred to WP-0078; macOS arm64 to WP-0080 |
| Windows loopback TCP compatibility | Blocked by environment | WP-0079; requires Windows native host/runner |
| Secure loopback binding | Satisfied locally | Loopback default and explicit non-loopback guard tested |
| Scoped authorization | Satisfied | Discovery, inference, administration, self/cross-client cancellation tests |
| Request IDs, cancellation, timeout, terminal, metrics | Satisfied locally | Checkpoints 5–8; cross-platform transport behavior remains open |
| Resource admission | Satisfied for current CPU-first policy | Context and conservative memory admission tested; accelerator-specific accounting remains future work |
| OID public API integration | Satisfied locally | Provider-neutral Marina client and interactive OID integration verified |
| Existing IDE compatibility edge | Partially satisfied | OpenAI-compatible native edge exists; broader IDE compatibility migration remains follow-up |
| Training gate before training work | Satisfied as a governance gate | ADR-0020 execution remains deferred; Phase 1 contracts may be planned |

## Environment blockers

- Linux x86_64: no Linux host/VM/container runtime; Docker daemon unavailable.
- Windows x86_64: no Windows host or Windows GitHub runner.
- macOS arm64: no Apple Silicon host.
- Remote transport: no authenticated private/TLS deployment environment.

Compilation or cross-compilation on macOS does not satisfy these criteria.

## Release decision

Marina may retain the qualified macOS x86_64 local-only release baseline. It
must not be advertised as cross-platform qualified, remote-ready, or suitable
for production network exposure. WP-0077 closes only after WP-0078, WP-0079,
and WP-0080 provide platform evidence and WP-0081 provides remote transport
evidence, or after a future governance decision explicitly narrows WP-0077's
scope and records the transfer.

WP-0078 is now satisfied for the tested Ubuntu 26.04 x86_64 AMD Phenom II X6
baseline. WP-0079, WP-0080, and WP-0081 remain open. The GitHub Actions Ubuntu
workflow is additional baseline evidence and does not replace the Phenom
result.
