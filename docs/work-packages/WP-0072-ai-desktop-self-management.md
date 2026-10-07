# WP-0072: AI Desktop Self-Management

## Purpose

Turn OID from a model-assisted console into a governed AI desktop/OS control
plane that can inspect, plan, operate, verify, and maintain the computer.

## Scope

- Desktop/system context graph with freshness and privacy policy.
- Typed skills for files, processes, packages, services, settings, devices,
  desktop surfaces, networking, and diagnostics.
- Risk classification, approval prompts, allowlists, leases, and privilege
  brokering.
- Plan execution with dependencies, bounded parallelism, cancellation,
  rollback, and verification.
- Health monitors and policy-approved low-risk self-maintenance.
- User-visible explanations, notifications, evidence, and recovery history.
- Evaluation fixtures for task success, unsafe-action refusal, recovery, and
  offline/model-unavailable behavior.

## Acceptance criteria

- A user can prompt a supported computer task and receive an inspectable plan.
- Low-risk allowlisted maintenance can run automatically within explicit
  budgets and permissions.
- Consequential actions pause for approval and show scope, impact, and
  rollback before execution.
- Failed verification routes to revision or rollback and preserves evidence.
- OID never grants a model direct shell, filesystem, privilege, or device
  authority.

## Dependencies

WP-0063, WP-0068, WP-0070, WP-0071; ADR-0004, ADR-0006, ADR-0016.
