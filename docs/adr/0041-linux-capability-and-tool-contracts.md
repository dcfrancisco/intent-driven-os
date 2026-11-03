# ADR-0041: Linux capability and tool contracts

- Status: Proposed
- Related: ADR-0003, ADR-0005, WP-0044, WP-0051, WP-0052, WP-0053

## Context
Existing Linux skills are typed read-only boundaries, while mutations need a
consistent contract.

## Decision
Every capability declares read/mutate operations, schema, scope, privilege,
failure behavior, audit, verification, and reversibility. Tool binaries remain
implementation details behind allowlisted adapters; arbitrary shell is not a
capability.

## Alternatives and consequences
Direct shell is flexible but unsafe and untestable. Typed adapters cost work but
permit least privilege and evidence. Existing read-only skills remain compatible.

## Migration, acceptance, open questions
Catalog current skills and add mutation one operation at a time. Accept with
schema, denial, scope, and postcondition tests. Open: complete package/network
capability set.
