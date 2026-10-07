# WP-0068: OID Runtime Integration

## Purpose

Make the OID console and governed system workflows first-class Marina clients.

## Scope

- OID client adapter over the public Marina API.
- Assistant session identity, system-operation policy, and evidence links.
- Model and backend capability discovery in OID.
- Same-machine local principal for OID, using Marina's single shared model
  inventory and loaded-model state.
- End-to-end tests covering standalone Marina and OID together.

## Acceptance criteria

- OID can use Marina when the interactive console and runner are installed
  separately.
- OID does not instantiate backend-native handles or read model files directly.
- OID sees models installed for Marina without copying them into an OID-owned
  directory or maintaining a second registry.
- Assistant/system calls preserve request identity, policy decisions,
  cancellation, metrics, and audit evidence across the boundary.

## Dependencies

WP-0063, WP-0066, WP-0067; ADR-0004, ADR-0010, ADR-0013.

## Implementation status (2026-10-07)

The shared model store and public service boundary are available for OID
integration, but the interactive `oid-console` still starts an in-process
runtime. The OID remote client adapter and end-to-end OID-to-Marina test are
not complete; this WP remains in progress and is not declared done.
