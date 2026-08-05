# WP-0010: Security Architecture

## Purpose

Define the identity, policy, permission, and approval architecture that protects model resources and intent-based system operations.

## Scope

Identity, authentication, authorization, least privilege, model and device permissions, secrets, approval flow, audit, transport security, plugin trust, and failure behavior.

## Deliverables

- Threat model and trust-boundary diagrams.
- Principal, identity, role, capability, and policy vocabulary.
- Permission matrix for model, backend, hardware, registry, and administrative actions.
- Approval flow for sensitive or destructive intent operations.
- Unix socket peer authentication and optional TCP/TLS security requirements.
- Security event, audit, redaction, and retention requirements.

## Acceptance Criteria

- Every privileged action has an authenticated principal and authorization decision.
- Least-privilege defaults are defined for local, service, console, and enterprise clients.
- High-impact operations require explicit approval or an auditable policy equivalent.
- Network exposure and plugin activation fail closed unless explicitly configured.
- Credential and sensitive prompt handling rules are documented.

## Dependencies

WP-0001, WP-0002, WP-0004, WP-0007, WP-0009; ADR-0006, ADR-0008.

## Future Work

Add hardware-backed identity, enterprise policy providers, tenant isolation, signed plugins/manifests, sandboxing, and security certification evidence.

