# WP-0087: Marina Training Artifact and Lineage Management

## Purpose

Manage immutable model, adapter, checkpoint, conversion, and deployment
artifacts across training and inference boundaries.

## Scope

- Artifact hashes, formats, base-model references, adapter compatibility,
  lineage, retention, and access metadata.
- Explicit conversion/package steps for inference formats.
- Candidate/active/rollback identities and evidence links.

## Acceptance criteria

- Every candidate identifies its base model, dataset, backend, and artifact
  hashes.
- Incompatible artifacts cannot activate.
- Partial or failed artifacts remain isolated from production models.
- Rollback restores a prior approved artifact without retraining.

## Dependencies

WP-0082, WP-0084, WP-0085, WP-0086, ADR-0020, WP-0077.

## Status

Planned. Artifact management only; no distributed training or pretraining.
