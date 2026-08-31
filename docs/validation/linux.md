# Linux Validation

## Platform support policy

OID has three distinct platform classifications:

- **Tier 1 — Linux:** the production deployment target. Linux-specific
  behavior may use `/proc`, process groups, Linux signals, systemd, D-Bus,
  Wayland, and Linux hardware discovery behind platform boundaries.
- **Tier 2 — macOS:** a supported development environment for the portable OID
  core. OID is not currently a macOS desktop product, and Linux-specific
  behavior must remain isolated behind platform abstractions.
- **Docker Linux validation:** a reproducible Linux build and integration
  environment. Docker validation is not equivalent to certification on a real
  Linux desktop or host.

## Docker validation

The initial validation image is Ubuntu 24.04 LTS with Rust stable, Cargo,
native Rust build tools, process utilities, and the dependencies needed by the
workspace.

From macOS or Linux, run:

```text
./scripts/validate-linux.sh
```

The command builds the image and runs, in order:

1. formatting verification;
2. workspace build;
3. strict workspace Clippy;
4. workspace tests;
5. the serial process/signal integration suite;
6. `git diff --check`.

Any failed command returns a non-zero status. Set
`OID_LINUX_VALIDATION_IMAGE` to override the local image tag. The entry point
is suitable for a future GitHub Actions job without requiring CI-specific
behavior.

The image is intentionally minimal and does not install desktop packages.
Debian and Fedora can be added later by introducing sibling image directories
and selecting the image in the same script; they are not part of the initial
matrix.

## WP-0056 Docker coverage

The container attempts to exercise:

- SIGINT and SIGTERM handling;
- SIGHUP shutdown;
- native-child interruption and repeated interruption;
- stubborn-child cleanup;
- governed-operation interruption during execution;
- executed-but-unverified recovery;
- evidence-boundary recovery;
- SIGKILL/restart recovery.

The process/signal suite runs serially because it starts and signals real OID
child processes. SIGKILL is treated only as recovery; it is never handled as a
graceful signal.

## Docker limitations

Docker validation does not prove:

- real login-TTY behavior or terminal-mode restoration under an interactive
  desktop terminal;
- normal host process-group and session behavior in every container runtime;
- a normal systemd manager or user session;
- a D-Bus user session;
- a Wayland compositor or desktop integration;
- complete host hardware visibility;
- reboot, host shutdown, or install/uninstall behavior on a real machine.

The container uses a private PID namespace and `--init` for basic child
reaping. Those semantics are useful for repeatable integration checks but are
not a substitute for host certification.

## Real Linux host checklist

Run this checklist on an Ubuntu host with a real interactive terminal before
claiming WP-0056 Linux validation complete:

- [ ] Real TTY behavior and terminal restoration after interruption.
- [ ] Repeated Ctrl+C while native commands are active.
- [ ] Process-group cleanup and no orphaned children.
- [ ] Interactive programs: `bash`, `ssh localhost`, `vim`, `less`, `top`.
- [ ] Long-running commands: `ping`, `sleep`, `yes`.
- [ ] Ordinary development tools: `git`, `cargo`.
- [ ] Long-running OID terminal session.
- [ ] SIGINT, SIGTERM, SIGHUP, and SIGKILL/restart scenarios.
- [ ] Systemd user-session behavior where applicable.
- [ ] D-Bus user-session behavior where applicable.
- [ ] Wayland behavior where applicable.
- [ ] Install, upgrade, and uninstall behavior.

Do not check these items based on Docker results alone.
