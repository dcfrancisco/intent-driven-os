# WP-0007: Command Grammar

## Purpose

Define how users express intent in a compact, discoverable console language while preserving explicitness, safety, and a path to literal system commands.

## Scope

Prompt syntax, intent text, structured arguments, target/resource references, command modes, ambiguity handling, dry runs, confirmation, errors, and examples only. No parser implementation.

## Deliverables

- Informal grammar and command taxonomy.
- Examples such as `summarize README.md`, `explain this folder`, `review changes`, and `why is nginx failing?`.
- Rules for explicit targets, destructive intent, confirmation, and cancellation.
- Plan/explain/execute presentation contract.
- Shell escape and compatibility policy.

## Acceptance Criteria

- Users can distinguish model-management commands from system-operation intents.
- Ambiguous or high-impact requests have a defined response path.
- Examples demonstrate read-only, diagnostic, creative, and administrative intents.
- Grammar decisions do not require a specific model or backend.

## Dependencies

WP-0002, WP-0006, WP-0010; ADR-0005, ADR-0006.

## Future Work

Define completion metadata, localization, aliases, machine-readable intents, and versioned grammar compatibility.

