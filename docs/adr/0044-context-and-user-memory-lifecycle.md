# ADR-0044: Context and user memory lifecycle

- Status: Proposed
- Related: ADR-0006, ADR-0015, ADR-0038

## Context
Conversation history is neither authoritative system state nor a safe default
for long-term retention.

## Decision
Separate system observations, current intent, execution history, preferences,
temporary context, derived knowledge, and audit records. Recheck system state
before mutation. Expose retention, deletion, and external-transmission controls.

## Alternatives and consequences
One transcript is simple but causes stale-state and privacy failures. Separate
stores require correlation and lifecycle policy but improve correctness.

## Migration, acceptance, open questions
Begin with no prompt retention by default and evidence references. Accept with
redaction, opt-out, deletion, and stale-state tests. Open: preference schema.
