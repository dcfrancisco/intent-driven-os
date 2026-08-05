# WP-0004: Model Registry Design

## Purpose

Define how the runtime identifies, trusts, stores, selects, and exposes installed models and their artifacts.

## Scope

Model manifests, versions, sources, licenses, backend compatibility, quantization variants, SHA256 verification, cache metadata, revocation, and inspection.

## Deliverables

- Manifest schema and example records.
- Registry state machine and source trust model.
- Cache layout, integrity, eviction, and migration design.
- Pull, install, inspect, and verification workflows.
- Policy for credentials, licenses, provenance, and revoked artifacts.

## Acceptance Criteria

- Every loadable artifact has an immutable identity and verified digest.
- A client can distinguish installed, cached, trusted, unavailable, and revoked states.
- Quantization and backend compatibility are expressible without parsing models in the runtime core.
- Failure and interrupted-download behavior are defined.

## Dependencies

WP-0002, WP-0003; ADR-0003, ADR-0006, ADR-0007.

## Future Work

Support signed manifests, mirrors, offline registries, encrypted caches, and enterprise registries.

