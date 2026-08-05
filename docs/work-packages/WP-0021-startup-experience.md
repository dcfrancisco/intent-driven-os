# WP-0021: Startup Experience

## Purpose

Make startup fast, legible, and representative of the Runtime + Console architecture.

## Scope

Startup banner, configuration, event bus, mock runtime, console readiness milestones, no-model message, help guidance, and initial status bar. Avoid excessive logging.

## Deliverables

- Startup sequence with configuration, event bus, runtime, and console milestones.
- Retro terminal banner and `int>` entry point.
- Explicit no-models-installed guidance.
- Startup integration test.
- Documentation of non-TTY and graceful-shutdown behavior.

## Acceptance Criteria

- Startup displays the requested AI Console and Intelligent Runtime identity.
- The sequence reports configuration loaded, event bus started, runtime initialized, and console ready.
- The user is told to type `help` to begin.
- Startup remains independent of model loading, hardware probing, and Linux operations.
- The application exits cleanly on EOF and Ctrl+C.

## Dependencies

WP-0016, WP-0018, WP-0019, WP-0020; ADR-0001, ADR-0004.

## Future Work

Add optional startup timing diagnostics, configurable banners/themes, model readiness details, and service-mode startup reporting.

