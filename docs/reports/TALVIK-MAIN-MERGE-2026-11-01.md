# Talvik feature branch and origin/main integration — 2026-11-01

## Outcome

Integrated remote main into `feature/qwen-coding-assistants` with a normal merge, preserving both histories. No rebase, reset or force-push is used. This is source integration and regression verification, not a new installed release, live IDE acceptance or RC promotion.

Inputs:

- Feature parent: `a8c66a4921c316d281a3a5d21bd7e2ba7f5b0e49`.
- Remote main: `8b7dd2e3c85e358facb8c24f8861898341d6a806`.
- Common ancestor: `d90bbe3af7a55c22bcf168d19c2103273ee64089`.
- Before integration: 35 commits only on the feature branch, 5 only on main (including merge commits). Main's net changes affected four files.

The feature branch matched its GitHub tip before integration. A complete-history bundle at the original feature tip was saved outside the repository and independently cloned and checked with `git fsck --full`. A disposable checkout confirmed the two content conflicts before the approved merge was applied. The source tree was clean before merging.

## Resolution decisions

`console/src/bin/http.rs` had overlapping lifecycle changes. The merge retains structured tool history, loaded model templates and parsers, reasoning separation, early stream acknowledgement, bounded socket timeouts, disconnect polling and RAII request cleanup from the feature branch. Main's assistant-role event builder is shared by the actual writer and its tests rather than creating duplicate stream-start events. Main's ten-second SSE heartbeat and comment payload are retained, while the short polling loop still checks disconnects and request deadlines. Redundant timeout/heartbeat handling was removed. First-token timing diagnostics are preserved without logging message content. Existing ownership reservation remains authoritative; no later unconditional insertion overwrites it.

`runtime/src/service.rs` conflicted in a recovery test fixture. Main's 1 GiB memory headroom is retained so eviction is driven by the one-resident policy rather than an impossible peak-load budget. Its zero idle retention, explicit wait for the generation slot to clear and eviction assertion are retained. This does not increase production memory limits.

`console/tests/support/mod.rs` merged automatically, retaining main's short `/tmp` fallback for macOS Unix-socket path limits. Strict Clippy caught a now-unnecessary clone after the fallback changed the local endpoint from a borrowed path to an owned `PathBuf`; returning that owned value fixes it. The affected transport tests were rerun after this lint correction. `docs/TALVIK-OPENAI-COMPATIBILITY.md` retains main's streaming lifecycle explanation, now matching the shared ten-second heartbeat constant. Other feature changes remain in the resulting tree.

## Verification

| Check | Result |
| --- | --- |
| Daemon/HTTP | 53 passed, 0 failed, 0 ignored |
| Runtime library/recovery | 51 passed, 0 failed, 0 ignored |
| IPC chat, signals and startup | 12 passed, 0 failed, 0 ignored |
| Full workspace including doc tests | 281 passed, 0 failed, 10 ignored |
| Formatting and staged source diff | Passed |
| Strict Clippy, console/runtime all targets | Passed with `-D warnings` |

Evidence: [merge logs](evidence/main-merge/talvik-merge-workspace-tests.log), [Clippy](evidence/main-merge/talvik-merge-clippy.log), and the [manifest](evidence/manifest.json). The initial lint failure is retained in `evidence/main-merge/talvik-merge-clippy-initial.log`; final strict lint passed after the owned-path correction. The full workspace run preceded that ownership-only correction, and the affected console suites plus lint passed afterward. Counts for the focused suites overlap with the full workspace total and must not be added to it. Ignored native fixtures remain unexecuted by the workspace test command.

Commands used Rust 1.88.0, `--locked`, the existing native library directory, and explicit required core/vision/chat-template build flags. Test commands were `cargo test -p oid-console --bin talvikd`, `cargo test -p oid-runtime --lib`, `cargo test -p oid-console --test startup --test ipc_chat --test process_signals`, `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy -p oid-console -p oid-runtime --all-targets -- -D warnings` (with the stated toolchain/lock flags).

## Remaining product work

The [implementation status](TALVIK-STATUS-2026-11-01.md) and [manual qualification gates](TALVIK-QUALIFICATION-RUNBOOK.md) remain applicable. This merge does not install the embedded-template release, prove a real build-only VS Code tool loop, qualify DeepSeek for Agent tools, remove the project-specific preset instructions, establish Ollama latency parity, or qualify additional platforms. No recurring test was recreated and the installed daemon was not restarted.
