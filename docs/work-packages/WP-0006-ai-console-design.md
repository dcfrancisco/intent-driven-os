# WP-0006: AI Console Design

## Purpose

Design a distinctive AI Console that feels like an evolution of classic DOS, UNIX, and VT100 terminals rather than a chat application or conventional terminal.

## Scope

Retro terminal UX, keyboard-first interaction, prompt design, startup/status view, streaming output, errors, cancellation, model/runtime state, and minimal mouse usage. No GUI.

## Deliverables

- Text-first screen and interaction specification.
- Prompt and status-line behavior, including `int>` or equivalent prompt treatment.
- Streaming output layout and input-state transitions.
- Keyboard shortcuts, history, completion, cancellation, and accessibility guidance.
- Console/runtime client boundary and example sessions.

## Acceptance Criteria

- The primary interaction is a prompt, not a giant conversation window.
- Runtime, backend, hardware, memory, model, and policy state can be shown compactly.
- Streaming responses do not corrupt the prompt or terminal state.
- A user can cancel, inspect, and understand an operation from the keyboard.
- The design explicitly excludes GUI implementation.

## Dependencies

WP-0002, WP-0007, WP-0009, WP-0010; ADR-0004, ADR-0005.

## Future Work

Add shell escape modes, configurable themes, panes, local history, remote sessions, and desktop embedding.



## Dot-prompt implementation, 2026-10-31

The console now uses a dBase-style `. ` prompt, a restrained two-line heading,
and one pinned status row with state/model only. No inline presence glyph or
startup model dump remains. Detailed state is requested through `:status` and
other inspection commands. `OID_STATUS_MODE=hidden` is supported; legacy `full`
no longer makes the default screen crowded.

Terminal input is event-driven, supports Unicode and a horizontal viewport,
and responds to resize. Native commands temporarily get the full terminal;
normal exit, cancellation and handled shutdown restore terminal settings.
The colon command namespace and OID approval/evidence boundaries are unchanged.

Executable verification: `scripts/test-oid-console-terminal.py` against built
clients, plus console unit/process tests. Qualification is local Intel macOS;
no other platform claim follows from these checks.
