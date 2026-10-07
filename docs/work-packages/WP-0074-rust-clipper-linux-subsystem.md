# WP-0074: Rust CA-Clipper Linux Subsystem

## Purpose

Bring the CA-Clipper application/domain investment into the Linux OID product
as a maintainable Rust subsystem.

## Phase 0: inventory and legal gate

- Preserve a read-only source snapshot and record provenance.
- Inventory `.PRG`, `.DBF`, `.NTX`, reports, batch files, libraries, binaries,
  schemas, and external integrations.
- Identify copyright, licensing, and redistribution constraints.
- Capture representative workflows and golden data fixtures without secrets or
  production records.

## Phase 1: target architecture

- Choose the migration target: compatible runtime, domain rewrite, data layer,
  or a combination.
- Define Rust crates, Linux UI/terminal boundary, storage adapters, import/
  export, reporting, and OID integration.
- Specify transaction safety, backup, migration, rollback, and audit behavior.

## Acceptance criteria

- Inventory and provenance report is complete.
- Legal/redistribution status is recorded before bundling any legacy asset.
- A selected workflow runs through a Rust/Linux vertical slice with tests.
- Legacy data can be imported/exported with verified round-trip fixtures.
- OID integration uses governed APIs and preserves approval/evidence boundaries.
- Linux distro packaging includes the Rust subsystem only after it passes the
  packaging and data-migration gates.

## Dependencies

ADR-0018, ADR-0016, WP-0069, WP-0072; external source inventory and legal
review.
