# WP-0075: Product Brand and Launch Language

## Purpose

Make the OID product family understandable and prevent product marketing from
over-claiming implementation status.

## Scope

- Product names, one-line promises, audience, and release boundaries.
- README, CLI help, installer, API metadata, screenshots, and website language.
- Implemented/preview/planned capability labels.
- Marina-to-OID product relationship and IDE onboarding language.
- Launch sequence from runner to AI desktop/OS and Linux distro.

## Candidate decision record — 2026-10-17

| Candidate | Strength | Observed risk | Disposition |
| --- | --- | --- | --- |
| Talvik | Distinctive, pronounceable, compact CLI | Public-name clearance unverified | Provisional engineering identity |
| Brixen | Compact and infrastructure-oriented | Geographic/name ambiguity | Reserve only |
| Korvex | Strong systems tone | Existing software and AI usage observed | Reject for now |
| Orvik | Compact and Nordic tone | Existing software/AI usage observed | Reject for now |
| Veltron | Strong runtime tone | Existing commercial usage observed | Reject for now |

This matrix is preliminary product screening. It is not trademark advice and
does not establish package, repository, or domain availability.

## Acceptance criteria

- A new user can distinguish Marina, OID, the OS-management model, and the
  Rust CA-Clipper subsystem.
- Every public claim maps to an implementation, ADR, or planned WP.
- The Marina installer clearly states its Linux/macOS/Windows scope.
- OID documentation clearly states its Linux-first scope.
- Marketing language does not imply unrestricted autonomous computer control.
- The provisional name and compatibility aliases are documented without
  blocking runtime delivery.
- Legal/trademark, package-registry, repository, and domain clearance has an
  explicit disposition before a public stable-name release.

## Dependencies

ADR-0019; WP-0067, WP-0068, WP-0072, WP-0073, WP-0074.
