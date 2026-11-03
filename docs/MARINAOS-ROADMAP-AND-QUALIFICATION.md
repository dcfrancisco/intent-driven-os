# MarinaOS roadmap and qualification matrix

| Milestone | Outcome | Gate | Current status |
| --- | --- | --- | --- |
| 1 | Services and Talvik installed on supported Ubuntu | clean-machine install, local inference, diagnostic proof | Packaging exists; qualification evidence incomplete |
| 2 | Reproducible image | pinned manifest and deterministic build | Image assets exist; not a release |
| 3 | Bootable artifact | VM boot, readiness, degraded AI behavior | Proposed/WP-0088 in progress |
| 4 | Installation and recovery | upgrade, rollback, data retention, recovery | Planned |
| 5 | Hardware matrix | CPU/GPU/network/storage/input qualification | CPU-first; real Linux evidence required |

## Acceptance status

The 24 requested acceptance scenarios are **proposed** unless covered by the
existing tests and reports. Existing evidence covers parts of intent lifecycle,
approval, durable operation recovery, read-only skills, Talvik model/runtime
contracts, package smoke checks, and hardware discovery. It does not yet prove
a complete natural-language-to-diagnosis-to-remedy workflow on a clean Linux
machine, nor a bootable MarinaOS release.

Required next evidence includes: clean Ubuntu package install and uninstall;
real local inference; local-only operation with OIP unavailable; truthful CPU
fallback; crash/replay/stall/cancellation tests; diagnostic receipt with
before/after measurements; and VM boot/recovery evidence.

The smallest independently verifiable slice delivered with this plan is the
`IntentProposal` contract and integration tests in `crates/intent-runtime`.
