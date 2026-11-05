# MarinaOS desktop baseline assessment

## Reusable functionality

- `marinaosd` owns a versioned read-only local socket with `status`, `doctor`,
  `hardware`, and `version` methods.
- `intent-runtime` owns intent lifecycle states and events.
- `operation-coordinator` composes policy, approval, execution, verification,
  rollback, and evidence, including a read-only system-health skill.
- `desktop-shell` contains input-routing and D-Bus transport contracts, but no
  concrete GTK or D-Bus adapter.
- Talvik is a separate optional model runtime and is not a privileged executor.

## Gaps found before UI work

There was no `apps/` desktop target, no GTK/libadwaita dependency, and no
authenticated user-session API for intent submission, approval, operation
events, verification, or evidence. The current MarinaOS daemon has no mutation,
shell, Talvik-control, or network IPC methods. Ubuntu and graphical-session
qualification are not complete.

## This slice

`apps/marina-desktop` connects to the real foundation IPC asynchronously and
renders only returned values. Request entry is deliberately not represented as
submitted work until the governed endpoint exists. This is a functional UI
foundation, not completion of the slow-computer acceptance demonstration.
