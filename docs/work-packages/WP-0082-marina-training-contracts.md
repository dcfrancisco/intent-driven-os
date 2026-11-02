# WP-0082: Marina Training Contracts

## Purpose

Define the backend-neutral Phase 1 training contracts required by ADR-0020
without executing training.

## Scope

- Training backend SPI and capability advertisement.
- Dataset, model, adapter, lineage, artifact, job, checkpoint, evaluation,
  promotion, and evidence schemas.
- State transitions and terminal failure taxonomy.
- Isolation boundary between training workers and production inference.
- API/CLI contract drafts using Marina's existing service and authorization
  conventions.

## Acceptance criteria

- Contracts do not expose framework-native types through Marina APIs.
- Training jobs cannot activate production models directly.
- Required hashes, provenance, permissions, resources, and correlation IDs are
  represented.
- Mock contract tests are specified separately from real training tests.

## Dependencies

ADR-0020, ADR-0021, WP-0077, WP-0058, WP-0070, WP-0071.

## Status

In progress. The first hardware-independent contract slice is implemented in
`runtime/src/training.rs`: explicit LoRA/QLoRA/full/scratch method identities,
backend capability and requested-device declarations, bounded training
configuration validation with no silent fallback, and deterministic job-state
transitions. Dataset identity and validation contracts are also implemented.

Backend process SPI, artifact/checkpoint/evaluation/promotion schemas, durable
job records, management API/CLI exposure, and worker execution remain planned.
No training backend or training execution is implemented by this slice.
