# Talvik model-development contract

Status: first internal contract and dataset slice, not a public API or training
capability. Governing decision: ADR-0020. Implementing work: WP-0082 and
WP-0083.

## Boundary

Talvik's model-development subsystem is distinct from the inference runtime.
It may register datasets, orchestrate isolated workers, record artifacts, and
run evaluation, but it cannot activate a production model as a side effect of
training. Framework objects, tensors, optimizers, native handles, and Python
types never cross Talvik's Rust or management API contracts.

The current implementation contains no worker and executes no training. It
adds only hardware-independent contracts and dataset persistence. The existing
GGUF/llama.cpp inference path is unchanged.

## Dataset format: `talvik.sft-jsonl/v1`

Input is a UTF-8 JSON Lines file. Blank lines and malformed records are errors;
they are never silently discarded. Each record has exactly one of these forms:

```json
{"prompt":"Question","completion":"Answer","split":"train"}
```

```json
{"messages":[{"role":"system","content":"Be concise"},{"role":"user","content":"Question"},{"role":"assistant","content":"Answer"}],"split":"validation"}
```

`split` is optional and defaults to `train`; only `train` and `validation` are
accepted. Prompt/completion text must be non-empty. Conversations must contain
non-empty `system`, `user`, or `assistant` messages and at least one user and
assistant turn. Tool messages, arbitrary content objects, executable dataset
code, and other schemas are unsupported and fail validation.

Conversational examples are rendered deterministically for duplicate detection
as a JSON array of ordered `[role, content]` pairs. Duplicate examples remain
in the immutable input but their count is recorded. Trainers must make an
explicit future policy decision about whether duplicates are admissible; the
registry does not silently rewrite source data.

Default limits are 256 MiB per artifact, 1,000,000 records, 1 MiB per record,
and 100 retained diagnostics. Callers may apply stricter limits. A validation
report includes total, training, validation and duplicate record counts plus
line-addressed diagnostics and an omitted-diagnostic count.

## Identity and persistence

Dataset IDs and versions use 1–128 ASCII letters, digits, `.`, `_`, or `-`.
An `(id, version)` pair is immutable. Repeating an identical import is
idempotent; changed content, provenance, or owner conflicts.

Validated bytes are copied into a managed `artifacts/sha256/<digest>.jsonl`
path, re-hashed before publication, and then referenced by an atomically
rewritten `registry.jsonl`. Restart opens fail closed on malformed metadata,
duplicate identities, paths outside the expected content address, missing
artifacts, or digest mismatch. The source path, provenance, owner, byte size,
registration time, partition counts, format, and digest are durable.

The current code exposes a `datasets:manage` scope constant, but no route or
CLI consumes it yet. Until authenticated composition and audit evidence exist,
the registry is an internal library capability and must not be advertised as a
Talvik management API.

## Training contracts

Training methods are declared independently as LoRA, QLoRA, full fine-tuning,
and from-scratch training. Declaring LoRA never implies QLoRA. A backend must
advertise its methods and device classes. Configuration validation rejects an
unsupported method or requested device and never selects a fallback.

The job-state contract is:

```text
PENDING -> VALIDATING -> QUEUED -> PREPARING -> RUNNING
RUNNING <-> CHECKPOINTING
RUNNING/CHECKPOINTING -> EVALUATING -> COMPLETED
non-terminal -> FAILED
non-terminal -> CANCELLING -> CANCELLED or FAILED
```

Terminal jobs cannot restart. This is a value-level state machine only; job
persistence, reconciliation, worker isolation, cancellation, checkpoints, and
execution belong to WP-0084 and are not implemented.

The implementation request used `CREATED`, while ADR-0020 and WP-0084 use
`PENDING`. This contract retains the repository decision's `PENDING` name and
adds the requested preparation, checkpointing, and cancellation substates;
the terminology conflict is not silently resolved by changing the ADR.

## Explicitly unavailable

- Derived-model creation and model copy/delete.
- Public dataset or training API and CLI commands.
- Training backend discovery or executable worker SPI.
- Real LoRA, QLoRA, full fine-tuning, or scratch training.
- Adapter import, activation, merge, or inference.
- Evaluation, comparison, promotion, deployment, and rollback.
- Checkpoints, export, conversion, quantization, and GGUF load qualification.
- Sensitive-content classifiers and external knowledge/RAG attachment.

These remain governed by WP-0084 through WP-0087 and must return unsupported
if exposed in a future edge before their production implementation exists.
