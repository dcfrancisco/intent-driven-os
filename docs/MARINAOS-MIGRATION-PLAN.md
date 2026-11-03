# MarinaOS consolidation and migration plan

## Repository inventory

| Existing component | Current source | Verified status | MarinaOS target | Migration |
| --- | --- | --- | --- | --- |
| Intent/lifecycle | `crates/intent-runtime`, `crates/common` | Implemented typed lifecycle; proposal contract added in this slice | Intent Engine | Retain crate; add versioned schemas incrementally |
| Policy/approval | `crates/policy-engine` | Implemented durable approval boundary | Policy Authority | Retain; add principal/scope contract |
| Orchestration | `crates/operation-coordinator` | Implemented durable journal, recovery classifications, verification flow | Agent Orchestrator | Retain ownership; expose transport-neutral API |
| Linux capabilities | `crates/linux-skills` | Read-only health, process, filesystem and service boundaries exist | System Services capability catalog | Retain; add typed schemas and mutation one capability at a time |
| Evidence/verification | `crates/evidence-engine`, `crates/verification-engine` | Implemented foundation | Supervisor/evidence plane | Retain; correlate receipts across services |
| Local inference | `runtime`, `crates/model-runner`, `runtime/adapters/llama-cpp` | Talvik runtime, native boundary, model lifecycle and streaming implemented to documented scope | Talvik package | Extract/package; do not move inference internals |
| Clients | `console`, `crates/model-client` | TTY and API clients exist | Marina UX clients | Compatibility rename only after accepted brand decision |
| Linux packaging/image | `packaging`, `image`, `scripts` | DEB/RPM/archive and image composition assets exist; boot qualification remains open | MarinaOS distribution track | Preserve workflows; qualify clean Linux/VM |
| External intelligence | No native Marina provider boundary yet | OIP references are documented, not local authority | Optional OIP adapter | Add after native local path is proven |

Unavailable repositories: no separate MarinaOS or Delivery OS repository was
accessible in this workspace. No source files in those projects are assumed.

## Migration rules

1. Keep OID names, binaries, paths, and APIs compatible until a separate brand
   decision is accepted.
2. Keep Talvik public inference compatibility stable; adapters translate at
   the edge.
3. Move valid components by ownership/documentation before rewriting them.
4. Add ADR and tests when an invariant or public interface changes.
5. Do not introduce a privileged daemon or unrestricted shell bridge as part
   of the naming consolidation.

## Executable sequence

1. Validate diagnostic proposals and route them to existing read-only skills.
2. Correlate diagnostic evidence into a user-facing receipt.
3. Add one reversible mutation with policy, approval, recheck, and verification.
4. Package the qualified services for Ubuntu; preserve Talvik standalone use.
5. Build and boot a reproducible image in a VM; only then expand hardware and
   installer qualification.

Rollback is a documentation/package rollback to the prior OID/Talvik release;
no repository rename, deletion, or data migration is part of this plan.
