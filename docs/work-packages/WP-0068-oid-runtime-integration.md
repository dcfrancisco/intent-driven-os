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

The provider-neutral `oid-model-client` contract and Marina HTTP adapter are
available, with the `oid-marina-chat` OID-side smoke client proving model
discovery, authenticated inference, invalid-credential handling, and daemon
unavailability handling against a real local GGUF. The interactive
`oid-console` now supports explicit `OID_RUNTIME_BACKEND=marina-http` mode;
the default remains the in-process runtime. The remote session wrapper keeps
OID operations local and routes only model discovery/inference through Marina.
Governed console cancellation/evidence integration and target-platform
verification remain open. Checkpoint 4B verified remote runtime events and
interactive cancellation/recovery; inference commands now create durable
inference start/terminal records in the existing append-only evidence log
without retaining prompts or responses. Checkpoint 5 verified metrics
propagation, cancellation terminal evidence, unavailable/auth failure
evidence, and retrieval after an OID process restart. A same-session
read-only system inspection still passes through OID policy and reports the
existing four-stage governed-operation evidence chain.
