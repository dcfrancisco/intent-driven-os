# WP-0113: MarinaOS Ubuntu Clean-VM Qualification

## Purpose

Execute the MarinaOS Phase 1 package gates on a clean Ubuntu 24.04 LTS amd64
VM and publish reproducible evidence before any image or installer work.

## Required evidence

- VM image identifier, kernel, firmware/virtual hardware, and exact package
  SHA-256.
- Offline DEB installation with no source checkout or compiler.
- sysusers/tmpfiles creation and unprivileged foundation service start.
- status, doctor, hardware, restart, and recovery logs.
- malformed configuration diagnostics.
- Talvik absent while foundation remains usable.
- package upgrade with user state preserved.
- package removal with documented state/conffile retention.

## Exit gate

Mark VM_TESTED only after all required checks pass and the report records
commands, timestamps, logs, and pass/fail outcomes. This package does not
authorize a bootable ISO or hardware-support claim; those belong to later
work packages.
