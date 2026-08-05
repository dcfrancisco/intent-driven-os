# ADR-0004: Runtime and Console Separation

- Status: Accepted

## Context

The runtime must be reusable by more than one user interface and must remain operable when no interactive console is present. The AI Console needs freedom to develop a focused retro terminal experience without becoming the owner of model state.

## Decision

The runtime contains no UI. The AI Console is one client of the runtime API. Future clients may include a desktop, REST API, background services, local applications, and enterprise integrations.

## Consequences

Runtime APIs, streaming semantics, authentication, and audit events must be designed for non-interactive clients. The console is simpler to reason about, but it must translate user intent into API requests and present runtime state without duplicating it.

## Alternatives Considered

- Embedding the runtime in the console would limit reuse and complicate headless operation.
- Building a UI-neutral monolith would still allow presentation concerns to leak into lifecycle and policy code.
- Treating the console as the only client would constrain future desktop and enterprise use.

## Future Considerations

Transport may begin with a Unix socket and later include an authenticated, TLS-protected TCP or REST surface. Client capability discovery and API versioning are required.

