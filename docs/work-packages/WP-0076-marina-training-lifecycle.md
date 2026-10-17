# WP-0076: Marina Training, Fine-Tuning, and Model Lifecycle

## Purpose

Implement ADR-0020 as a governed, backend-neutral training lifecycle without
coupling the inference runner to PyTorch, Transformers, PEFT, TRL, or another
training framework.

## Scope

- Training backend SPI and capability registry.
- Dataset identity, immutable versions, schema validation, provenance, and
  content hashes.
- Training-job persistence, state transitions, cancellation, checkpoints,
  worker isolation, retries, and resource limits.
- CPU-compatible small-model training proof.
- Model/adapter lineage and artifact compatibility.
- Reproducible evaluation, baseline comparison, promotion approval, activation,
  and rollback.
- Hardware/resource eligibility and inference priority.
- Training metrics, audit evidence, and CLI/API observability.
- Later LoRA/QLoRA integration behind the same contracts.

## Non-goals

- Full-parameter foundation-model training.
- Automatic training from user conversations or OS observations.
- Automatic production promotion.
- Unrestricted training-worker host access.
- Making training dependencies mandatory for Marina inference.

## Acceptance criteria

- A versioned dataset can be registered, validated, and hashed.
- A real CPU-compatible backend can execute a bounded training job.
- A failed or cancelled job leaves the active inference model unchanged.
- A completed job produces an unpromoted candidate with lineage and hashes.
- Evaluation produces reproducible evidence against a named baseline.
- Promotion requires policy/approval and activation is rollback-compatible.
- Resource admission rejects unsupported hardware, memory, disk, or concurrency.
- Mock orchestration tests are separated from real training/inference tests.
- OID can authorize governed training without granting the worker OS authority.

## Dependencies

WP-0058, WP-0060, WP-0062, WP-0070, WP-0071, WP-0073, WP-0077; ADR-0020,
ADR-0021.

## Phase 1 decomposition

Implementation is intentionally split into WP-0082 through WP-0087:

- WP-0082: contracts and isolation boundary;
- WP-0083: dataset registration and validation;
- WP-0084: job lifecycle and worker isolation;
- WP-0085: CPU-compatible training backend;
- WP-0087: artifact and lineage management; and
- WP-0086: evaluation and promotion gates.

## Status

Proposed. No training endpoints or training backend are implemented by this WP
yet; the existing Marina inference path remains independently usable. Planning
may proceed, but execution remains gated by the Phase 1 contracts and
production-inference isolation requirements.
