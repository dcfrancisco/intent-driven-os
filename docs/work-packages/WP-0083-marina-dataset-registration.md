# WP-0083: Marina Dataset Registration and Validation

## Purpose

Register explicitly selected, immutable, provenance-bearing training datasets
without collecting prompts, responses, or OS observations automatically.

## Scope

- Dataset identity, version, schema, content hash, provenance, access policy,
  validation result, and retention metadata.
- Local artifact references and safe import boundaries.
- Authorization and evidence for registration and validation.

## Acceptance criteria

- Invalid, mutable, or incompatible datasets are rejected before job execution.
- Dataset records survive restart and are content-addressed.
- Sensitive data is not copied into logs or model artifacts by default.

## Dependencies

WP-0082, ADR-0020, WP-0077, existing evidence persistence.

## Status

Planned. Registration and validation only; no training execution.
