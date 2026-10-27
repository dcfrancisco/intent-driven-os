# WP-0107: Talvik Per-User Windows MSI

## Status

Proposed. The current Windows release is a ZIP containing the Talvik binaries,
native DLLs, and a PowerShell installer. This work package plans an additional
per-user MSI; no MSI implementation or Windows runtime qualification is
claimed.

## Purpose

Add a conventional Windows installer artifact for the Talvik model runner
without changing Talvik's per-user installation boundary or requiring OID,
MarinaOS, administrator privileges, or a Windows service.

## Scope

- Select and pin a maintained MSI authoring toolchain and the supported
  Windows runner/tool versions.
- Add creation of a versioned `talvik-windows-x86_64.msi` to
  `.github/workflows/talvik-release.yml`, initially alongside the existing
  ZIP and PowerShell installer.
- Package `talvik.exe`, `talvikd.exe`, the matching llama.cpp/ggml native DLLs,
  license files, version metadata, and installation metadata. Do not bundle
  model files.
- Install for the current user under `%USERPROFILE%\.marina` by default,
  retaining the `bin`, `lib`, `models`, `state`, and `logs` layout.
- Define whether user-`PATH` integration is offered, its default, and how the
  installer reverses only its own change. Machine `PATH` must not be modified.
- Define how the MSI handles required Microsoft C/C++ runtime components
  without silently introducing an administrator-only prerequisite.
- Define version identity, upgrade and repair behavior, and a safe response
  when daemon or DLL files are in use. Do not forcibly terminate unrelated
  processes.
- Generate and publish a SHA-256 checksum. Require Authenticode signing for
  stable public releases; keep signing credentials out of source and ordinary
  build logs.
- Keep Windows runtime qualification in WP-0079; MSI artifact success alone
  does not qualify inference, cancellation, timeout, or recovery.

## Non-goals

- Replacing or removing the existing ZIP plus PowerShell installer before the
  MSI has passed acceptance and migration is explicitly approved.
- MSIX or per-machine installation.
- Installing or enabling a Windows service, auto-starting `talvikd`, or
  modifying machine-wide environment variables.
- Installing OID, MarinaOS, an operating-system image, or model artifacts.
- Downloading models or adding provider/network credentials.
- Claiming Windows runtime qualification from packaging tests alone.

## Acceptance criteria

- The Windows release workflow produces the MSI from the same versioned
  Talvik binaries and matching native-library build used for the Windows
  release artifacts.
- The MSI installs in the current-user context without administrator
  privileges, writes no files under `Program Files`, and does not modify
  machine-wide registry or `PATH` state.
- The installed layout lets `talvik.exe` and `talvikd.exe` load the matching
  native DLLs using the documented Windows loader layout.
- A clean install supports `talvik.exe --version` and `talvik.exe --help`;
  the daemon can start and report its documented local health state.
- Upgrade and repair preserve models, configuration, state, and logs. A
  running daemon or locked native DLL produces a clear, recoverable outcome.
- Uninstall removes only MSI-owned application files and optional user-PATH
  entries. It preserves `%USERPROFILE%\.marina\models`, configuration,
  state, and logs.
- CI validates MSI contents, checksum, signing state, clean install, upgrade,
  and uninstall using a temporary user prefix or isolated Windows runner.
- The installer does not bundle models, start a service, or trigger model
  downloads.
- Stable public MSI releases have verifiable Authenticode signatures. Any
  unsigned development artifact is clearly marked and is not presented as a
  stable signed release.
- Documentation clearly distinguishes the Talvik installer from OID and
  MarinaOS and links packaging evidence separately from WP-0079 runtime
  qualification.

## Validation plan

1. Build the Windows x86_64 native llama.cpp payload and Talvik binaries from
   the pinned revisions on the supported Windows runner.
2. Inspect the MSI table/file manifest to verify intended paths, architecture,
   product identity, permissions, and included DLLs.
3. Install under a clean standard user without elevation; verify the installed
   CLI and daemon start and load their adjacent native DLLs.
4. Create representative model, configuration, state, and log files; perform
   an upgrade and repair; verify all user data remains unchanged.
5. Test upgrade while the daemon is active and verify the documented
   non-destructive handling of locked files.
6. Uninstall and verify only MSI-owned binaries/libraries and optional
   user-PATH entries are removed.
7. Verify SHA-256 and Authenticode signature using Windows tooling.
8. Run WP-0079 runtime qualification separately before making Windows runtime
   support claims.

## Dependencies

ADR-0030; ADR-0012, ADR-0013, ADR-0021, ADR-0024; WP-0064, WP-0065, WP-0079,
WP-0090; the existing Windows build and release workflow.

## Decisions to resolve before implementation

- MSI authoring technology and pinned version.
- Microsoft C/C++ runtime delivery strategy.
- Whether to offer per-user `PATH` integration and whether it is enabled by
  default.
- Product and upgrade identity/versioning policy.
- CI signing ownership, certificate storage, and stable-release signing gate.
- The supported Windows versions and clean-runner qualification matrix.

## Next dependency

Review ADR-0030 and select the open installer/runtime/signing details above.
Then update the Windows release workflow and add isolated MSI lifecycle smoke
tests without removing the current ZIP installer.
