# ADR-0008: Plugin Architecture

- Status: Accepted

## Context

Backend engines and future capabilities will evolve at different rates and may have different dependencies, licenses, hardware requirements, and failure modes. The core runtime should not become a monolith containing every integration.

## Decision

Backends and future capabilities are implemented through versioned plugins or adapters behind stable runtime contracts. The core owns discovery, capability negotiation, policy, lifecycle, health, and trust decisions. Extensions do not bypass the core security, resource, or audit paths.

## Consequences

The ecosystem can grow independently and optional integrations can be isolated. The project must define compatibility, packaging, process boundaries, signing, permissions, crash handling, and extension lifecycle before treating plugins as trusted.

## Alternatives Considered

- Compile every backend into the core increases distribution size and coupling.
- Unrestricted dynamic plugins create an unacceptable trust and privilege boundary.
- Separate one-off integrations in every client duplicate behavior and weaken consistency.

## Future Considerations

Evaluate in-process versus out-of-process adapters, plugin manifests, signing and revocation, sandboxing, resource quotas, ABI/API stability, and enterprise-managed extension policy.

