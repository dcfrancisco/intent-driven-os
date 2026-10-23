# Talvik M2 Capability Closeout

Date: 2026-10-23
Branch: `feature/talvik-m2-runtime-ide`
Starting revision: `eabc2e8`
Final implementation revision: `a96bb3a`
Reference local runner reference: `f52b9e742d8c9a308f7b23cd1f0ab52c7ddb694d`

## Decision

M2 is **not fully qualified**. This implementation slice is reviewable and
verified by deterministic checks, but real IDE sessions, native cross-platform
runs, Docker runtime execution, and full lifecycle qualification remain open.
The complete inventory is maintained in
[`TALVIK-COMPATIBILITY-CAPABILITY-MATRIX.md`](TALVIK-COMPATIBILITY-CAPABILITY-MATRIX.md).

## Implemented P0

- Build identity for `talvik` and `talvikd`: package version, Git SHA, dirty
  state, target, profile, JSON output, HTTP `/version`, and IPC daemon/client
  mismatch fields.
- Existing multi-model residency, FIFO single-generation scheduling, bounded
  queue, cancellation, memory admission, LRU recovery, pinning, and restart
  reconciliation remain preserved.
- Existing authenticated OpenAI endpoints, embeddings, streaming, tool
  handling, `/api/tags`, and `/api/ps` remain preserved.

## P0 remaining

- CAP-001 through CAP-004 and CAP-006 through CAP-012 require native or live
  daemon qualification beyond deterministic tests.
- CAP-022 and CAP-023 require real Continue and Cline sessions. They are
  blocked by unavailable external IDE sessions, not silently passed.

## P1 and P2 disposition

- CAP-013 through CAP-015 are deferred to WP-0096 until a real client requires
  the additional Reference local runner routes.
- CAP-018 and CAP-019 are owned by WP-0097; no full OCI/content-addressed
  distribution is authorized for M2.
- CAP-020 remains serialized under WP-0094 pending native measurements.
- CAP-021 remains partial under WP-0091 while counters and wait-time evidence
  are expanded.
- CAP-025 through CAP-027 are scheduled for M3.
- CAP-028 and CAP-029 are explicitly out of M2 scope and remain scheduled for
  separately authorized milestones.

## Work-package traceability

| Work package | Scope | Status |
|---|---|---|
| WP-0091 | Runtime diagnostics and `/api/tags`, `/api/ps` | Implemented, incompletely qualified |
| WP-0092 | Continue/Cline/Codex qualification | Blocked/deferred pending clients |
| WP-0093 | Retention, pinning, resource policy | Implemented, incompletely qualified |
| WP-0094 | Concurrent inference decision | Deferred pending native measurements |
| WP-0095 | Build/runtime identity | Implemented; deterministic verification passed |
| WP-0096 | Additional Reference local runner endpoints | Deferred pending client demand |
| WP-0097 | Model profiles and registry integrity | Scheduled M2.1 |
| WP-0100–0103 | GPU, multimodal, MarinaOS, alternate engines | Scheduled later |

## Verification evidence

- `cargo fmt --all`: passed.
- `cargo test -p oid-console --bin talvik --bin talvikd`: passed (5 tests).
- `cargo clippy -p oid-console --all-targets --all-features -- -D warnings`:
  passed.
- `talvik --version --verbose`, `talvik --json version`,
  `talvikd --version --verbose`, and `talvikd version --json`: passed.
- Final release binaries built from `a96bb3a` with `dirty=false`: `talvik`
  SHA-256 `5d10974b8d273eaa8f2b0722b0357fc943db0aaa037fbb3ee4f42cb9aeac8fa4`;
  `talvikd` SHA-256
  `841c7723b9cac2c513b7a8a79fa87d1fc298983042dd8fd2a073de0664106c53`.
- Full workspace regression, native GGUF qualification, live daemon mismatch,
  and HTTP endpoint execution remain separate gates.

## Platform and client matrix

| Environment | Native runtime | IDE/client | Status |
|---|---|---|---|
| macOS x86_64 | Prior M1 CPU-first GO; M2 deterministic checks pass | No real Continue/Cline session | Incompletely qualified |
| macOS arm64 | Not executed | Not executed | Not verified |
| Linux x86_64 | Not executed | Not executed | Not verified |
| Windows x86_64 | Not executed | Not executed | Not verified |
| Docker CPU | Compose config validated; runtime not executed | N/A | Blocked |

## Remaining limitations

No claim is made for real IDE interoperability, unexecuted native hosts,
Docker runtime behavior, per-request keep-alive, concurrent generation, or
additional Reference local runner routes. Public release remains **NO-GO** until mandatory
qualification evidence is collected.
