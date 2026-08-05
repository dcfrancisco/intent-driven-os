# ADR-0002: Rust as the Primary Implementation Language

- Status: Accepted

## Context

The runtime and console are long-lived Linux systems software. They need reliable concurrency, explicit failure handling, predictable resource behavior, strong boundaries between privileged operations, and maintainable cross-platform adapters.

## Decision

Use Rust as the primary implementation language. Organize the project as modular Cargo crates with explicit types and traits at subsystem boundaries. Forbid unsafe Rust by default; any exception requires a documented architectural decision and focused review.

## Consequences

Rust provides memory safety without a garbage collector, strong compile-time contracts, explicit error handling, mature testing and tooling, and suitable async foundations. A modular workspace supports isolated testing and reviewable security boundaries. The project accepts a higher up-front cost for ownership, lifetimes, API design, and crate coordination.

## Alternatives Considered

- C/C++ offer mature systems and inference ecosystems but make memory safety and ownership boundaries harder to enforce.
- Go simplifies concurrency and deployment but provides weaker compile-time modeling for some resource and capability boundaries.
- Python is valuable for experimentation but is not the preferred foundation for a resource-managed, security-sensitive runtime.

## Future Considerations

Foreign-function interfaces may be required for inference engines or hardware SDKs. Such code must remain behind narrow adapters, with safety wrappers, lifecycle ownership, and platform-specific testing.

