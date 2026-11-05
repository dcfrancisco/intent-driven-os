# WP-0116: MarinaOS native desktop UI foundation

## Outcome

Provide the first Rust GTK 4/libadwaita MarinaOS user-session shell. It
connects asynchronously to marinaosd's real read-only IPC and renders
foundation status, hardware, Talvik optionality, request navigation, and
explicit disconnected/unavailable states.

## Scope and gate

`apps/marina-desktop` is in scope. No compositor, voice process, companion,
shell execution, or privileged D-Bus is in scope. SOURCE_PRESENT and
UNIT_TESTED apply to adapters/parser; GRAPHICAL_TESTED requires a real Linux
graphical session. The slow-computer end-to-end scenario is NOT COMPLETE until
a governed API exposes intent, diagnostics, approval, execution, verification,
and evidence.

## Next dependency

WP-0117 should define the authenticated user-session intent and operation event
API, reusing intent-runtime, operation-coordinator, policy-engine,
verification-engine, and evidence-engine.
