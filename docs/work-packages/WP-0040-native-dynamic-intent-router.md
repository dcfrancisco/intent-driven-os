# WP-0040: Native/Dynamic/Intent Input Router

## Status

Complete — Milestone 4.

## Outcome

`oid-desktop-shell` now exposes `InputRouter` and `InputRoute`. Native command
names are preserved as executable-plus-arguments, registered capabilities are
selected by command or alias, and all other input remains an intent request.

The router classifies input only; it does not execute commands or bypass policy.
