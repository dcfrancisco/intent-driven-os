# WP-0041: Dynamic Capability Registration

## Status

Complete — Milestone 4.

## Outcome

Capabilities can be registered and unregistered at runtime through
`CapabilityRegistry`. Registration requires a stable provider, capability
identity, owning skill, mutability declaration, descriptions, and completion
metadata. Unregistration removes new command resolution immediately.

In-flight execution lifecycle and evidence remain the responsibility of the
governed operation boundary introduced in WP-0038.
