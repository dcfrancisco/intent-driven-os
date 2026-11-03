# ADR-0052: Installation, packaging, and release qualification

- Status: Proposed
- Related: ADR-0046, ADR-0050, ADR-0051

## Context
Release artifacts must install without a local compiler or llama.cpp build
environment.

## Decision
Qualify native DEB first, RPM where compatible, and per-user archives across
target architectures. Each release includes checksums, native libraries,
service identities, permissions, model-storage policy, clean-machine install,
real inference, upgrade, uninstall, and evidence.

## Alternatives and consequences
Source-only installation excludes intended users. More artifacts increase CI and
signing work but make support claims measurable.

## Migration, acceptance, open questions
Integrate existing Talvik release workflows; do not replace them. Accept clean
installation and end-to-end inference before declaring a package supported.
Open: release repository and signing ownership.
