# WP-0028: Model Discovery

## Purpose

Make local GGUF artifacts visible to the persistent model registry without
loading them.

## Scope

Recursive configurable directory traversal, `.gguf` filtering, stable IDs,
basic file metadata, duplicate avoidance, and registry events.

## Deliverables

- `ModelDiscovery` service.
- Default directories: user data, `~/Models`, and `./models`.
- Registry population tests for GGUF and non-GGUF files.

## Acceptance Criteria

Only GGUF files are registered; discovery never calls the backend loader.

## Dependencies

WP-0024, WP-0027.

## Future Work

Manifest extraction, SHA256 verification, cache policy, and split-model support.
