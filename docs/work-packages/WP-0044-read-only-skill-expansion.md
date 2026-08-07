# WP-0044: Read-only Skill Expansion

## Status

Complete — Milestone 4.

## Outcome

`oid-linux-skills` now includes governed directory inspection and file metadata
inspection skills. Both produce canonical non-mutating plans, execute only
through approved plans, verify their result, and explicitly report that
rollback is unavailable because no system state changed.
