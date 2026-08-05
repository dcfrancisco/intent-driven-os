# ADR-0006: Security by Default

- Status: Accepted

## Context

The runtime may manage local models, hardware, filesystem artifacts, remote providers, and operations that affect a Linux system. An insecure local default can become a serious boundary failure when the runtime is reused by services or exposed over a network.

## Decision

Security is enabled by default. The architecture requires authentication, authorization, least privilege, auditable actions, explicit policy decisions, and safe handling of model artifacts and credentials. There are no insecure defaults: network exposure, broad permissions, unrestricted model loading, and silent privileged actions require explicit configuration and policy.

## Consequences

Initial setup and API design are more deliberate. Every operation carries identity and policy context, and the system must retain useful audit records without leaking sensitive prompts or model data. Secure defaults reduce convenience for experimental deployments but make later service and enterprise use viable.

## Alternatives Considered

- Trusting local users or Unix socket access alone is insufficient for multi-user and future networked operation.
- Adding security after the runtime is functional risks incompatible APIs and unreviewed privilege paths.
- A permissive development mode as the default makes unsafe behavior easy to carry into production.

## Future Considerations

Define credential storage, sandboxing, plugin trust, TLS, tenant isolation, artifact signing, secret redaction, retention, and fail-closed behavior when policy services are unavailable.

