# WP-0039: Operation Registry and Allowlist

## Status

Complete — Milestone 4.

## Outcome

`oid-plugin-sdk` now provides a thread-safe `CapabilityRegistry`. Hosts reserve
native command names, validate capability metadata, reject collisions, and
enumerate loaded capabilities deterministically.

The registry is metadata-only: registration never executes plugin or skill code.
Execution remains behind the existing approved operation-plan lifecycle.
