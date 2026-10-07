# ADR-0017: OID OS-Management Model Track

- Status: Proposed
- Date: 2026-10-07
- Related: ADR-0003, ADR-0005, ADR-0006, ADR-0015, ADR-0016; WP-0073

## Decision

OID will have a dedicated model-development track for an OS-management model.
The model is trained or fine-tuned specifically for computer operation: system
and desktop context interpretation, plan generation, typed tool proposals,
policy awareness, verification, recovery, and concise user communication.

This model is a consumer of the Marina runner. It is not allowed to bypass OID
policy, approvals, evidence, or skill boundaries. The runner remains a
backend-neutral serving layer and may serve this model alongside other local or
remote models.

“Build from scratch” is a research/product track, not a reason to block the
working system. The first useful OID model may be a specialized checkpoint
created through continued pretraining, supervised fine-tuning, preference/data
training, or distillation, provided its provenance and evaluation are recorded.
A genuinely new base model requires separate data, compute, licensing,
evaluation, and release gates.

## Model requirements

- Structured plan and typed tool-call output.
- Explicit uncertainty, refusal, and approval-needed states.
- Context handling for desktop, processes, files, services, devices, and
  runtime health without exposing unnecessary private data.
- Recovery-oriented reasoning and verification of postconditions.
- Offline/local operation with graceful degradation when the model is absent.
- Evaluation against safe action, refusal, recovery, latency, resource use, and
  task success—not only text quality.

## Boundary and release

The model never receives raw unrestricted shell authority or privileged
credentials. OID supplies scoped context and receives typed proposals. A model
release must include dataset/data-governance records, model-card provenance,
license status, safety evaluations, supported context/tool schema, resource
requirements, and a rollback-compatible version identity.

Marina releases are cross-platform for Linux, macOS, and Windows. OID and the
AI desktop/OS are Linux-first and Linux distro delivery remains their target;
the model itself may be served wherever Marina is supported.

This ADR is a product direction. No from-scratch OS-management model is
implemented or claimed by the current repository.
