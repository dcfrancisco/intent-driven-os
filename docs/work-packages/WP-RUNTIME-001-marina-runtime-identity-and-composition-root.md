# WP-RUNTIME-001 — Marina Runtime Identity and Composition Root

Status: complete (2026-09-05)

This work package normalizes the existing single-process application before an
IPC split. It does not add a daemon, client, routing, or a new model layer.

## Rule

There is exactly one production composition root:

```text
Runtime::start
  → MarinaRuntime
  → BackendManager
  → ModelRunner / backend adapters / state
```

The console is an in-process client of `RuntimeService`. It no longer depends
on a production type named `MockRuntime`. The llama.cpp adapter and the model
runner contract are unchanged.

## Implementation

- `Runtime::start` now delegates to the real backend/model/hardware service
  constructor instead of creating empty foundation providers.
- `Runtime::start_with_bus` supports the console's in-process event bus while
  using the same service constructor.
- The former mock-named service is exported as `MarinaRuntime` from the
  `service` module. Its real llama.cpp registration and lifecycle behavior are
  unchanged.
- The default runtime identity is now `Marina`.

The current executable remains `oid-console` until the later client/daemon
work package. IPC and executable renaming are intentionally out of scope here.
