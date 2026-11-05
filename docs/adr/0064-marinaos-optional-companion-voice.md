# ADR-0064: MarinaOS optional companion and voice processes

- Status: Accepted as a boundary
- Related: ADR-0015, ADR-0038, ADR-0053, WP-0116

Companion and voice processes are optional clients, outside this milestone.
They never own execution authority and must use the same typed intent and
approval contracts as the desktop. Their absence cannot prevent startup.
