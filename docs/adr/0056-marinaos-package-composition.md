# ADR-0056: MarinaOS package composition

- Status: Proposed
- Related: ADR-0050, ADR-0052, ADR-0053, WP-0114

marinaos-core contains only the unprivileged marinaosd daemon, its systemd
unit, configuration, sysusers, and tmpfiles assets. marinaos-cli contains
marinaosctl and depends on the exact matching core version. Talvik is not a
dependency of either package.

This split makes the service boundary independently installable and preserves
the existing command name. Both packages are amd64 DEBs for the initial
Ubuntu 24.04 qualification baseline.
