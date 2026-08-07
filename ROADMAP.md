# Roadmap

## Foundation — complete

- Establish the Rust workspace and crate boundaries.
- Document architecture, contribution, security, and governance expectations.
- Define the first architectural decision.
- Keep AI behavior unimplemented while operation contracts mature.
- Define shared identifiers, errors, and cross-context events.
- Define intent lifecycle transitions and invariants.
- Define policy, skill, verification, evidence, plugin, model, and shell traits.

## First implementation slice — complete

- Add deterministic in-memory policy, evidence, and verification test doubles.
- Implement the Linux `/proc` health adapter and portable fallback.
- Connect the intent lifecycle to the existing runtime event bus.
- Add durable append-only file evidence storage.
- Add the first explicitly approved, rollback-capable directory operation.

## Operation Planning Framework — complete

- Define canonical operation plans, steps, risk, approval, verification, and rollback models.
- Require approved plans for skill execution.
- Render inspectable plans in the terminal.
- Migrate system-health and create-directory flows to the common lifecycle.

## Capability governance and routing — complete

- Register and unregister dynamic capabilities through a validated registry.
- Protect native command names from silent dynamic shadowing.
- Route prompts into native CLI, dynamic capability, or intent paths.
- Expose loaded capability help and completion metadata.

Completed work packages: WP-0039, WP-0040, WP-0041, and WP-0042.

## Safe Linux operations — complete

- Add durable approval records and crash recovery for interrupted operations.
- Add more read-only skills before expanding mutating capabilities.

Completed work packages: WP-0043 and WP-0044.

Milestone 4 is complete. The next milestone should integrate these boundaries
into a persistent operation coordinator and terminal execution adapter.

## Persistent operation coordination — complete

- Compose planning, policy, approval, execution, verification, and evidence.
- Persist operation lifecycle transitions in an append-only journal.
- Add governed native command and dynamic capability adapters.
- Correlate lifecycle evidence to the canonical operation ID.
- Expose recoverable operations for a future explicit resume command.

Completed work packages: WP-0045 through WP-0050.

The next milestone should integrate the coordinator into the interactive console
and add explicit rollback and resume commands with user-visible recovery plans.

## Linux operations and desktop adapter boundaries — complete

- Add governed process-table and filesystem capacity inspection skills.
- Add an injectable, read-only systemd health adapter.
- Add an isolated read-only D-Bus transport contract for desktop adapters.
- Keep platform operations behind plans, verification, evidence, and explicit policy.

Completed work packages: WP-0051 through WP-0054.

The next milestone should add persistent recovery/resume plans and concrete
Wayland/D-Bus adapters behind these contracts.

## Additional safe Linux operations

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
