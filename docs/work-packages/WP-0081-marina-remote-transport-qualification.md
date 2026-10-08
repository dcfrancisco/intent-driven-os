# WP-0081: Marina Remote Transport Qualification

## Purpose

Qualify remote Marina deployment separately from local-only service readiness.

## Scope

- Select and document an authenticated private transport or mutually
  authenticated TLS boundary.
- Test OID and assistant clients across network interruption, timeout,
  cancellation, retry, credential rejection, and endpoint unavailability.
- Verify authorization scopes, evidence correlation, resource admission, and
  secret redaction across the remote boundary.
- Confirm no direct unauthenticated LAN/public exposure.

## Non-goals

- Public hosting or multi-tenant service design.
- Cloud BYOK implementation.
- Changes to Marina's internal provider-neutral runtime contracts.

## Acceptance criteria

- Remote inference succeeds only through authenticated private transport.
- Network failures produce bounded, recoverable client outcomes.
- TLS/identity, credential rotation, and deployment policy are documented.
- Local-only behavior remains unchanged.

## Dependencies

WP-0077, ADR-0021, WP-0071, and a controlled remote test environment.

## Status

Blocked: no authenticated remote deployment environment is currently available.
