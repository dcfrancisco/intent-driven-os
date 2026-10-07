# WP-0066: Standalone Service Transports

## Purpose

Make Marina usable as a standalone local service by OID, assistants, and
other applications.

## Scope

- Versioned health, hardware, backend, model, generation, stream, and
  cancellation resources.
- Unix socket transport on Linux/macOS and named-pipe transport on Windows.
- Loopback HTTP transport with explicit bind and exposure policy.
- Authentication/peer identity, request IDs, idempotency, limits, and graceful
  shutdown/restart behavior.

## Acceptance criteria

- OID and a non-OID client can use the same documented API.
- Streaming, cancellation, errors, and health behavior are equivalent across
  supported local transports.
- No transport exposes backend-native types.
- Remote/network access is disabled unless explicitly configured and secured.

## Dependencies

WP-0057, WP-0059, WP-0062, WP-0065; ADR-0011, ADR-0013.
