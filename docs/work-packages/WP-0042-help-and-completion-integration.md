# WP-0042: Help and Completion Integration

## Status

Complete — Milestone 4.

## Outcome

Loaded capabilities expose descriptions, aliases, and argument completion
candidates through `InputRouter::help_lines` and `InputRouter::complete`.
Completion is scoped to currently registered capabilities and returns sorted,
deduplicated command names.

Native shell completion remains outside this metadata boundary and can be
merged by a future terminal adapter.
