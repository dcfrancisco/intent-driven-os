# ADR-0062: MarinaOS UI state and execution state

- Status: Accepted
- Related: ADR-0039, ADR-0043, ADR-0049, WP-0116

UI selection, focus, loading, and presentation are ephemeral. Intent, approval,
operation, verification, rollback, and evidence are authoritative backend
state. The UI never infers success from model text or widget animation.
