# ADR-0060: MarinaOS user-session UI boundary

- Status: Accepted
- Related: ADR-0042, ADR-0053, ADR-0057, WP-0116

The UI runs as a per-user session process. Privileged or system-owned actions
remain in independently supervised services and typed capability adapters. UI
callbacks never execute shell commands or privileged D-Bus methods. The desktop
can disappear without taking down the foundation and cannot grant Talvik OS
execution authority.
