# ADR-0020: Marina Model Training, Fine-Tuning, and Lifecycle Management

- Status: Proposed
- Date: 2026-10-08
- System: Marina Model Runner
- Related system: Intent-Driven OS (OID)
- Category: Model runtime / machine-learning infrastructure
- Priority: P1 architecture, phased implementation
- Related work: WP-0076, WP-0077, WP-0082–WP-0087; ADR-0010, ADR-0011, ADR-0013, ADR-0015, ADR-0017, ADR-0021

## Context

Marina is the cross-platform local model execution layer for Linux, macOS,
and Windows. It owns model loading, inference, lifecycle management, and
model-serving APIs. OID owns intent interpretation, planning, authorization,
action execution, verification, and operating-system authority.

Model adaptation can make Marina more useful for specialized tasks, routing,
structured decisions, and OID's future OS-management model. Training has
different resource, security, reliability, and lifecycle requirements from
inference, so it must not be added as an uncontrolled extension of the serving
process.

Training depends on stabilization of Marina's transport-independent API,
request lifecycle, model registry, and resource admission. Training SHALL use
those shared platform capabilities rather than implementing separate network,
authorization, cancellation, or resource-management mechanisms. ADR-0021 and
WP-0077 therefore precede this lifecycle implementation.

## Decision

Marina SHALL gain a governed Training and Model Lifecycle Subsystem. It SHALL
provide, through replaceable backend adapters:

1. Training-job orchestration and cancellation.
2. Dataset registration, immutable versions, validation, provenance, and hashes.
3. Small-model training and parameter-efficient LoRA/QLoRA where supported.
4. Checkpoints, recovery, resource limits, and hardware-aware scheduling.
5. Model and adapter versioning, lineage, and compatibility checks.
6. Reproducible evaluation against defined acceptance criteria.
7. Controlled candidate promotion, activation, monitoring, and rollback.
8. Training evidence and audit records.

Training SHALL NOT automatically modify an active production model. A
completed training job produces an unpromoted candidate; evaluation and
deployment approval remain separate lifecycle decisions.

Marina remains a model runtime and orchestration component. Training
algorithms are delegated to isolated, replaceable integrations such as
PyTorch, Hugging Face Transformers, PEFT, TRL, or suitable small-model
libraries. Those dependencies must not become requirements of the core
inference runtime.

## Architectural boundaries

OID owns intent interpretation, authorization, human approvals, selection of
governed training inputs, operating-system actions, and verification of
system-level outcomes. OID does not require training to perform core functions
and must not gain direct access to training workers or backend-native handles.

Marina owns training-job lifecycle, backend selection, hardware discovery,
dataset/model compatibility, artifact registration, evaluation execution,
promotion/rollback state, resource limits, and workload isolation. Marina does
not gain unrestricted operating-system authority through training.

Training backends own gradient computation, optimizers, LoRA/QLoRA logic,
checkpoint serialization, and framework-specific acceleration. Their types and
handles remain behind Marina's backend-neutral interfaces.

## Logical components

- **Training Job Manager:** creation, validation, queueing, execution,
  cancellation, retries, recovery, and state transitions.
- **Training Backend Registry:** supported model families, methods, hardware,
  platforms, and artifact formats.
- **Dataset Registry:** immutable dataset identities, schemas, provenance,
  validation, access policy, and content hashes.
- **Hardware Capability Manager:** CPU, memory, accelerator, disk, and
  eligibility reporting.
- **Model and Adapter Registry:** base-model references, adapter compatibility,
  artifact hashes, versions, and lineage.
- **Evaluation Engine:** reproducible suites, baseline comparison, regression
  thresholds, output-schema checks, quality, latency, and memory checks.
- **Model Promotion Manager:** approval, atomic activation where supported,
  deployment history, and rollback.
- **Training Evidence Store:** configuration, inputs, backend versions,
  checkpoints, resources, metrics, evaluation, and release decisions.

All components expose stable interfaces independent of individual ML
frameworks.

## Standard lifecycle

```text
Register dataset
  -> validate dataset and base model
  -> select eligible backend and resources
  -> submit and authorize job
  -> isolate and execute training
  -> checkpoint and record evidence
  -> register unpromoted candidate
  -> evaluate against baseline
  -> human/policy approval
  -> activate candidate
  -> monitor and retain rollback
```

The job states are `PENDING`, `VALIDATING`, `QUEUED`, `RUNNING`,
`EVALUATING`, and `COMPLETED`; terminal failure states are `FAILED` and
`CANCELLED`. Failed or interrupted jobs cannot change the active inference
model.

## Phased training scope

### Phase 1: small models

Prioritize CPU-compatible classification, intent/task categorization,
routing/ranking, structured prediction, small neural networks, and decision
models where learning provides value beyond deterministic rules.

### Phase 2: parameter-efficient fine-tuning

Support LoRA through a compatible backend. QLoRA is conditional on model,
framework, accelerator, and available-memory support. Adapters retain the
compatible base-model identity and version.

### Phase 3: advanced training

Full-parameter, multimodal, distributed, multi-device, random-initialization,
and additional optimizer/quantization strategies remain future capabilities.

## Artifact compatibility and promotion

Training and inference formats are separate concerns. A backend-produced model
or adapter is not assumed compatible with every Marina inference backend. GGUF
deployment may require adapter merging, conversion, and quantization; every
conversion produces a new immutable artifact identity and preserves lineage.

Activation requires compatibility checks and evaluation. Required evaluations,
where applicable, include task quality, baseline regression, inference
compatibility, latency, memory, load reliability, output-schema compliance,
and task-specific safety/robustness. Promotion creates an auditable decision;
rollback restores the previous approved model without retraining. Automatic
promotion is disabled by default.

## Cross-platform and resource policy

Marina retains Linux, macOS, and Windows portability, but each training backend
advertises its supported platform, architecture, accelerator, method, and
artifact format. Unsupported configurations fail with actionable diagnostics.

Training workers run independently from inference wherever practical. The
system provides memory/CPU/accelerator limits, maximum concurrency, disk-space
checks, cancellation, checkpoint retention, exhaustion handling, and
inference priority. Remote training is a future explicitly configured and
authorized mode, not a fallback.

## Data security and governance

Marina SHALL NOT automatically convert prompts, responses, logs, or operating-
system observations into training data. Dataset registration requires explicit
selection or a separately authorized workflow. Provenance, access, encryption,
retention, and sensitive metadata policy apply to datasets, checkpoints,
artifacts, and evidence. Credentials must never be embedded in artifacts or
training evidence.

## Proposed API contracts

These are proposed contracts, not implemented endpoints. They must be
reconciled with Marina's established versioned API before implementation:

| Operation | Purpose |
| --- | --- |
| `POST /v1/datasets` | Register dataset metadata and artifact reference |
| `GET /v1/datasets` | Query datasets |
| `POST /v1/training/jobs` | Submit a training job |
| `GET /v1/training/jobs/{id}` | Inspect job state |
| `POST /v1/training/jobs/{id}/cancel` | Cancel a job |
| `GET /v1/training/backends` | Discover supported methods |
| `GET /v1/models/{id}/versions` | List versions and lineage |
| `POST /v1/evaluations` | Start candidate evaluation |
| `GET /v1/evaluations/{id}` | Retrieve evaluation results |
| `POST /v1/models/{id}/promotions` | Request promotion |
| `POST /v1/models/{id}/rollback` | Restore an approved version |

Long-running operations return job identities rather than holding request
connections open. These endpoints do not authorize training or promotion by
themselves; OID policy and Marina resource policy remain in force.

## Evidence and failure handling

Each job records its identifier, base-model and dataset hashes, backend/version,
configuration, seed where supported, hardware, timestamps, metrics,
checkpoints, artifact hashes, evaluation, and promotion decision. Exact
cross-hardware numerical reproducibility is not promised.

The subsystem explicitly handles insufficient memory, unsupported methods,
invalid datasets, missing artifacts, worker termination, disk exhaustion,
corrupt checkpoints, timeouts, conversion failure, evaluation failure, and
promotion rejection. Partial artifacts remain isolated, recovery uses
validated checkpoints where supported, and repeated failures cannot create
uncontrolled restart loops.

## Alternatives considered

1. **Inference-only Marina:** retained as a valid minimal deployment, rejected
   as the long-term capability boundary.
2. **Training algorithms inside Marina:** rejected in favor of mature,
   replaceable ML frameworks.
3. **External training only:** rejected as the sole model lifecycle because it
   loses common registry, evidence, compatibility, and promotion controls;
   external artifact import remains supported.
4. **Autonomous self-training:** rejected because automatic feedback
   incorporation and replacement create unacceptable quality and governance
   risks.

## Implementation plan

WP-0076 is the umbrella work package. Phase 1 is decomposed into the following
reviewable work packages, all dependent on the stabilized Marina API and
admission boundary in WP-0077:

1. WP-0082 — Training backend SPI, job states, datasets, lineage, artifacts,
   and the production-inference isolation boundary.
2. WP-0083 — Dataset registration, immutable versions, validation,
   provenance, access policy, and hashes.
3. WP-0084 — Job persistence, scheduling, cancellation, checkpoints, worker
   isolation, retries, and resource limits.
4. WP-0085 — A real bounded CPU-compatible small-model backend.
5. WP-0087 — Immutable model, adapter, checkpoint, conversion, and deployment
   artifact lineage.
6. WP-0086 — Evaluation, baseline comparison, promotion approval, activation,
   rollback, and evidence.

LoRA/QLoRA, hardware reservations, and richer training observability remain
follow-on work after these Phase 1 contracts and lifecycle controls. The
training worker SHALL remain isolated from the production inference process;
training SHALL NOT be added to the inference request path.

## Minimum acceptance criteria

The first usable release must demonstrate a versioned dataset, validation,
real CPU-compatible training, a versioned candidate, reproducible evaluation,
baseline comparison, unchanged active model after failure, approved activation,
rollback, and traceable evidence. Tests must distinguish mocked orchestration
from actual training and inference integration.

The initial implementation does not attempt distributed foundation-model
training, automatic retraining from every interaction, unrestricted host
access, bypass of OID approval, or automatic promotion.

## Final decision

Marina SHALL support training as a pluggable, governed model lifecycle
capability with strict separation between inference, training, evaluation, and
deployment decisions:

```text
Register -> Train -> Evaluate -> Approve -> Deploy -> Monitor -> Rollback
```
