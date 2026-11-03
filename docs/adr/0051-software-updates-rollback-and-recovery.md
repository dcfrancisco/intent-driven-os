# ADR-0051: Software updates, rollback, and recovery

- Status: Proposed
- Related: ADR-0006, ADR-0023, ADR-0043

## Context
AI services and models are stateful and upgrades must not destroy user data or
claim success after interruption.

## Decision
Sign and checksum release artifacts; separate binaries, configuration, models,
and evidence; preserve user state across upgrades; record version/build identity;
support service rollback and operation reconciliation. Package removal does not
purge model/evidence data without an explicit retention choice.

## Alternatives and consequences
In-place replacement is simple but unrecoverable. Staged updates add disk and
release complexity while making failures observable.

## Migration, acceptance, open questions
Extend existing package smoke checks. Accept interrupted upgrade, rollback,
retention, and corrupted-artifact tests. Open: image-level A/B strategy.
