# WP-0055: Terminal Identity and Shell Passthrough

## Status

Complete — Milestone 7.

## Product correction

OID Console is a Linux shell augmented by an intelligent runtime. Ordinary
shell commands remain ordinary shell commands. Intelligence is invoked
explicitly or through governed context; the console must not become a closed
command REPL or chatbot.

## Interaction model

```text
int> ls                         -> configured Linux shell
int> cargo test                 -> configured Linux shell
int> :status                    -> OID command handler
int> :intent diagnose-postgresql -> intent runtime
int> :quit                      -> OID command handler
```

Input beginning with `:` is routed to OID. Everything else is passed to the
user's shell, preserving shell pipelines, redirection, quoting, expansion,
environment, working directory, standard streams, and exit status.

## Requirements

- Resolve the shell in this order: `$SHELL`, `/bin/bash`, `/bin/sh`.
- Execute shell input with the shell command execution option.
- Preserve the console working directory across commands.
- Handle `cd`, `cd <path>`, and `cd -` in a persistent working-directory manager.
- Keep Ctrl+C scoped to the active child process and Ctrl+D as empty-line exit.
- Preserve command history and up/down navigation.
- Display shell exit status and handle missing commands without ending the console.
- Default runtime status display to compact, with compact/full/hidden modes.
- Keep OID commands namespaced: `:help`, `:status`, `:runtime`, `:models`,
  `:intent <description>`, and `:quit`.

## Components

- `InputRouter`
- `OidCommandHandler`
- `ShellExecutor`
- `WorkingDirectoryManager`
- `ChildProcessController`
- `PromptRenderer`
- `HistoryStore`

## Acceptance tests

- `ls` routes to the shell.
- `:help` routes to the OID handler.
- Unknown OID commands produce an OID-specific error.
- `cd` changes the persistent working directory.
- Failed shell commands preserve the console session.
- Pipelines and redirection execute correctly.
- Ctrl+C terminates only the active child process.
- Compact, full, and hidden prompt/status rendering work as configured.

## Explicit non-goals

No model integration, chatbot behavior, or automatic intent interpretation is
part of this work package.

## Implementation notes

The default status mode is compact. Set `OID_STATUS_MODE=full` or
`OID_STATUS_MODE=hidden` to change automatic status rendering. The shell
executor inherits standard streams for interactive commands and exposes a
captured-output path for tests and non-interactive clients.
