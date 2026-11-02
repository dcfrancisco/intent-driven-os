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

In progress. `runtime/src/training.rs` implements an internal production
registry for explicitly selected UTF-8 SFT JSONL files. It validates bounded
prompt/completion and conversational records, explicit train/validation
partitions, malformed records, encoding, size/record limits, and duplicates;
publishes SHA-256-addressed immutable artifacts; persists metadata atomically;
and verifies managed paths and artifact integrity on restart.

The registry is not yet composed into `talvikd`, exposed through authenticated
management API/CLI operations, connected to audit evidence or sensitive-data
checks, or qualified on the cross-platform matrix. Those are required before
WP-0083 can be called tested or complete. No training execution is included.
