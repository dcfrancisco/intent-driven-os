# WP-0073: OID OS-Management Model

## Purpose

Develop, evaluate, package, and serve the model specialized for governed OID
computer management.

## Scope

- Define the OID context, plan, tool-proposal, approval, verification, and
  recovery training schemas.
- Build governed datasets from synthetic and reviewed computer-operation
  traces; document privacy, licensing, and retention.
- Establish baseline prompting, continued pretraining/fine-tuning,
  preference/data training, and distillation experiments.
- Evaluate task success, unsafe-action refusal, approval compliance,
  verification quality, recovery, latency, memory, and offline behavior.
- Publish model cards, provenance, supported schemas, resource variants, and
  Marina manifests.
- Integrate the selected checkpoint with Marina without granting direct OS
  authority.

## Acceptance criteria

- The model emits validated typed proposals rather than unrestricted commands.
- OID approval and evidence gates remain effective under adversarial prompts.
- Evaluations demonstrate safe planning, verification, and recovery on a
  representative Linux task suite.
- The model has reproducible provenance and a rollback-compatible identity.
- Marina can serve the model as one selectable model variant.

## Dependencies

WP-0063, WP-0070, WP-0071, WP-0072; ADR-0015, ADR-0016, ADR-0017.
