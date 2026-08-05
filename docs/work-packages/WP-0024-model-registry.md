# WP-0024: Model Registry

## Purpose

Manage persistent model metadata independently from model downloads, parsing, loading, and inference.

## Scope

Model identity, name, family, backend, quantization, context window, memory requirement, capabilities, status, checksum, location, and list/register/remove/inspect operations.

## Deliverables

- Persistent file-backed `ModelRegistry`.
- Metadata value model and lifecycle status.
- Atomic mutation persistence and deterministic ordering.
- In-memory registry for mock runtime tests.
- Model registration events and console inspection output.

## Acceptance Criteria

- Metadata survives registry reopen.
- Duplicate and unknown IDs return structured errors.
- Registry operations do not download, parse, load, or infer models.
- Model metadata is exposed to clients through runtime interfaces.

## Dependencies

WP-0004, WP-0008, WP-0018, WP-0020; ADR-0003, ADR-0006.

## Future Work

Add signed manifests, checksum verification, cache integration, schema migration, revocation, and multi-tenant ownership.

