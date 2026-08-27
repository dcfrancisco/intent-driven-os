# Roadmap

This roadmap is frozen around one product proof: a user can express, approve,
execute, verify, inspect, and safely recover one governed operation from the
OID terminal. That proof is complete in `v0.1.0-alpha.1`. New architecture and
additional skills remain backlog work while the desktop foundation stabilizes.

## Completed foundations

- Rust workspace and bounded crate architecture.
- Runtime/console separation and explicit `:`-prefixed OID commands.
- Canonical operation plans with policy, approval, execution, verification,
  rollback, and append-only evidence boundaries.
- Durable approval, operation, and evidence journals.
- Native/dynamic/intent routing contracts.
- Read-only process, filesystem, and systemd inspection boundaries.
- Shell passthrough and persistent console working directory.

See the ADR index and work-package index for the historical design record.

## Current milestone: End-to-End Governed Operation UX

Status: complete — tagged `v0.1.0-alpha.1`.

### Golden path

```text
:intent create a directory /tmp/oid-demo
  -> deterministic intent parser
  -> inspectable OperationPlan
  -> explicit user approval
  -> execution
  -> verification
  -> durable evidence
  -> :operations inspect <id>
```

### Definition of done

- The complete path works from one terminal session.
- Every lifecycle transition survives process restart.
- The journal and evidence store reconstruct the operation history.
- Resume and rollback cannot bypass approval or policy.
- Integration tests deliberately interrupt execution at important lifecycle
  boundaries and validate safe recovery.

The milestone does not require an LLM. `:intent` uses a deterministic adapter
for the supported directory intent while the runtime and model remain absent.

## Next milestone: Desktop Foundation Stabilization

Goal: prove that OID can be trusted as an everyday terminal/runtime without
losing state, corrupting operations, or interfering with normal Linux behavior.

No new product capabilities are in scope. Work is organized into six gates:

1. **WP-0056 — Process and signal reliability** — Ctrl+C, Ctrl+D, SIGTERM/SIGHUP,
   child-process cleanup, interrupted governed operations, abnormal termination,
   and clean shutdown.
2. **State and corruption resilience** — truncated evidence, malformed state,
   duplicate IDs, partial writes, unavailable storage, incompatible versions,
   and safe startup when recovery data is damaged.
3. **Recovery torture tests** — terminate OID before approval, during execution,
   after execution and before verification, during evidence persistence, and
   during rollback; restart after each case and prove deterministic recovery.
4. **Native terminal compatibility** — preserve shell exit codes, pipelines,
   redirection, environment variables, `cd`, background processes, interactive
   programs, `ssh`, `git`, `cargo`, editors, resize, Unicode, and large output.
   OID must not make ordinary CLI behavior worse.
5. **Packaging and installation** — reproducible release binary,
   install/uninstall paths, configuration and state directories, permissions,
   upgrades, version reporting, and eventually `.deb`/`.rpm` packages.
6. **Real Linux validation** — Ubuntu-first smoke testing outside the test
   harness, including long-running use, deliberate termination, reboot,
   repeated governed operations, and recovery.

Milestone gate:

- No new skills, model training, dynamic CLI, phone, voice, plugin expansion,
  or richer desktop UI.
- Yes to reliability, recovery, tests, packaging, installation, diagnostics,
  and documentation.
- Validate on an actual Linux environment with interruption, restart,
  intentional corruption, long-running sessions, and ordinary shell use.

## `v0.1.0-alpha.2` release gate

The next tag is allowed only when all of these pass:

- Workspace tests, strict Clippy, and diff checks.
- Restart/recovery, corruption, signal, and native-shell compatibility suites.
- Clean-machine installation and uninstall with no unexpected system changes.
- Real Linux smoke testing with no model installed and no network connection:
  OID starts, the native terminal works, deterministic governed operations work,
  recovery works, and evidence works.

## Release sequence after stabilization

- `v0.1.0-alpha.1` — End-to-End Governed Operation Foundation.
- `v0.2` — Stable terminal and recovery hardening.
- `v0.3` — Concrete Linux desktop integration.
- `v0.4` — Local intelligence runtime.
- `v0.5+` — Expanded skills, dynamic CLI, and richer intent interpretation.

Phone pairing, voice, the Linux Operations Model, generated CLIs, historical
runtimes, and broad skill expansion remain explicitly deferred.

## Separate runtime track

The model/runtime sequencing remains documented in
[`runtime/ROADMAP.md`](runtime/ROADMAP.md) and [`runtime/BACKLOG.md`](runtime/BACKLOG.md).
It is not a dependency of the current governed-operation milestone.
