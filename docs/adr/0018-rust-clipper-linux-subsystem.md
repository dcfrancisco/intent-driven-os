# ADR-0018: Rust CA-Clipper Linux Subsystem

- Status: Proposed
- Date: 2026-10-07
- Related: ADR-0002, ADR-0003, ADR-0006, ADR-0016; WP-0074

## Context

The existing CA-Clipper project is a separate legacy archive at
`/Users/dannyfrancisco/codes/clipperProject/clipper`. It contains Clipper
source and historical build artifacts (`.PRG`, `.DBF`, `.OBJ`, `.EXE`, `.LIB`,
headers, and linker/tool files) and is not currently a Rust workspace.

OID is Linux-first and will need more than a model runner to become a complete
AI desktop/OS. The Clipper project may become a Linux business-application,
data, or compatibility subsystem within that distro, but legacy binaries and
toolchains must not be copied into the modern runtime without provenance and
licensing review.

## Decision

Create a Rust rewrite/compatibility track for the CA-Clipper project as a
Linux-only OID subsystem. Marina remains cross-platform; the Rust Clipper
subsystem is part of the Linux OID/distro product and is not a Marina backend.

The migration begins with inventory and behavior characterization, then
defines a clean Rust domain/API boundary. It may reuse documented data formats
or import/export compatibility where legally and technically appropriate, but
does not automatically translate every historical implementation detail.

The subsystem must integrate with OID through explicit services and governed
operations. It must not grant the model direct database, file, or business
transaction authority; model proposals pass through OID policy, approval,
execution, verification, and evidence.

## Initial investigation gates

- Confirm ownership, licenses, third-party components, and redistribution
  rights for the legacy archive.
- Identify the business capabilities and workflows that must survive.
- Inventory DBF/NTX data schemas, reports, batch jobs, terminal UI, and
  external integrations.
- Define whether the target is a source-compatible language/runtime,
  application rewrite, data compatibility layer, or a combination.
- Establish import/export, backup, migration, and rollback rules before
  modifying production data.

This ADR does not claim that the legacy project has been imported, translated,
or made runnable on Linux. It establishes the Linux product direction and the
required discovery gates.
