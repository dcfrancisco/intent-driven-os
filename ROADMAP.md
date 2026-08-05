# Roadmap

## Foundation — current

- Establish the Rust workspace and crate boundaries.
- Document architecture, contribution, security, and governance expectations.
- Define the first architectural decision.
- Keep all runtime and AI behavior unimplemented.

## Contracts and lifecycle

- Define shared identifiers, errors, configuration, and structured event contracts.
- Specify the intent state machine and transition invariants.
- Define typed skill, policy, verification, evidence, plugin, and shell traits.
- Add contract tests and deterministic in-memory test doubles.

## Safe Linux operations

- Implement a small, allowlisted set of read-only skills first.
- Add explicit authorization and approval decisions.
- Add postcondition checks, rollback contracts, and append-only evidence records.
- Integrate systemd and D-Bus through isolated Linux adapters.

## Desktop integration

- Build terminal event interfaces and a Wayland-first shell adapter.
- Surface plans, approvals, progress, results, and undo paths.
- Add crash recovery and evidence inspection.

## Intelligence and ecosystem

- Add model-runner interfaces only after operation contracts are stable.
- Evaluate local model execution and the Linux Operations Model.
- Introduce plugin discovery and capability isolation.
- Explore Open Intelligence Platform (OIP) integration.

