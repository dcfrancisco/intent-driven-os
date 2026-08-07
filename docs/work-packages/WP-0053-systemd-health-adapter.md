# WP-0053: Systemd Health Adapter

## Status

Complete — Milestone 6.

## Outcome

`SystemdHealthSkill` uses an injectable `SystemdHealthAdapter`. The default
adapter invokes only `systemctl is-system-running`; tests can supply a fake
adapter without requiring systemd on the host.
