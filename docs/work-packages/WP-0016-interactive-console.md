# WP-0016: Interactive Console

## Purpose

Transform the startup prototype into a responsive, keyboard-first terminal client for the Intelligent Runtime.

## Scope

Interactive prompt, line editing, cursor movement, history navigation, backspace, delete, Home, End, Ctrl+C, Ctrl+L, graceful shutdown, resize-safe terminal rendering, and placeholder built-in commands. No model, llama.cpp, hardware, or Linux-operation integration.

## Deliverables

- Terminal input adapter with raw-mode and non-TTY fallback.
- Editable line buffer and command history.
- Built-in commands: `help`, `status`, `history`, `clear`, `version`, `about`, and `exit`.
- Prompt and output renderer with ANSI-safe redraw behavior.
- Unit tests for line editing and history; integration test for startup.

## Acceptance Criteria

- The application launches into an interactive `int>` prompt.
- Arrow keys, editing keys, Ctrl+C, Ctrl+L, and EOF behave predictably.
- Commands return deterministic mock information and `exit` shuts down cleanly.
- Non-TTY execution does not hang automated tests.
- Console depends on runtime interfaces and does not implement runtime policy or inference.

## Dependencies

WP-0006, WP-0007, WP-0018, WP-0020, WP-0021; ADR-0004, ADR-0005.

## Future Work

Add completion, multi-line input, shell escape mode, terminal resize-aware layout, and streaming output integration.

