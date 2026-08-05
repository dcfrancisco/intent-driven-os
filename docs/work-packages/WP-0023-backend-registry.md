# WP-0023: Backend Registry

## Purpose

Provide a runtime-owned `BackendManager` for registering, enabling, disabling, discovering, selecting, and health-checking backend adapters.

## Scope

In-process registrations, enabled state, active backend selection, health lookup, event publication, and future plugin readiness. No dynamic loading or plugin execution yet.

## Deliverables

- `BackendManager` registry and selection API.
- Backend registration summaries with enabled/active state.
- Mock backend implementation.
- Events for registration, enablement, and disablement.
- Error behavior for duplicate, missing, and disabled backends.

## Acceptance Criteria

- Multiple adapters can be registered without client coupling.
- Only enabled backends can become active.
- Backend health is queried through the manager.
- Event bus receives all registry state changes.
- The console can list and inspect backend state through `RuntimeService` only.

## Dependencies

WP-0022, WP-0018; ADR-0003, ADR-0008.

## Future Work

Add plugin discovery, signed manifests, persistence, priority/fallback selection, and adapter process supervision.

