# ADR-0028: Per-User MSI Packaging for Talvik on Windows

- Status: Proposed
- Date: 2026-10-27
- System: Talvik Model Runner
- Related: ADR-0012, ADR-0013, ADR-0021, ADR-0024; WP-0064, WP-0079,
  WP-0090, WP-0094

## Context

Talvik is the independently installable, cross-platform model runner. Windows
support applies to Talvik; OID remains Linux-first, and MarinaOS is not a
Windows product.

The current Windows release workflow builds a ZIP containing `talvik.exe`,
`talvikd.exe`, llama.cpp/ggml DLLs, and `install-model-runner.ps1`. The script
installs the payload into the current user's `%USERPROFILE%\.marina` directory
without administrator privileges. It preserves model and runtime data during
uninstall. This is a working user-scoped archive installer, not an MSI.

The release process needs a reviewable proposal for producing a conventional
Windows installer while retaining Talvik's existing user-owned data layout,
native-library requirements, and separation from OID.

## Proposed decision

Create a signed, per-user Windows MSI for Talvik, initially as an additional
release artifact alongside the existing ZIP installer. Keep the ZIP available
until the MSI has passed its packaging and lifecycle acceptance criteria.

The MSI SHALL:

- Install only the Talvik model runner and its matching native DLL payload.
  It SHALL NOT install OID, MarinaOS, or an operating-system image.
- Install for the current user under `%USERPROFILE%\.marina` by default,
  preserving the existing `bin`, `lib`, `models`, `state`, and `logs` layout.
- Require no administrator privileges and make no machine-wide `Program
  Files`, service, registry, or `PATH` changes.
- Not install or start a Windows service, auto-start the daemon, download
  models, or initiate network access.
- Preserve user models, configuration, state, and logs across upgrades and
  uninstall. Uninstall removes only installer-owned binaries and native
  libraries.
- Keep the packaged llama.cpp/ggml libraries version-matched with the runner
  binaries. Native DLL placement must remain compatible with Windows DLL
  search behavior.
- Publish a SHA-256 checksum. Stable public MSI releases SHALL be
  Authenticode-signed using signing material held outside the repository.

Any optional user-`PATH` integration must be limited to the current user,
reversible on uninstall, and must not alter machine `PATH`. Its default
behavior is left for WP-0094 to resolve before implementation.

The installer authoring tool, exact Microsoft C/C++ runtime distribution
strategy, and upgrade behavior when Talvik binaries are in use must be
selected and pinned in WP-0094. Custom actions that require elevation or
perform unrelated system changes are not permitted.

## Alternatives considered

- **Continue with ZIP plus PowerShell only:** remains the supported baseline
  and is appropriate for portable/manual installation, but does not provide
  standard MSI install, repair, and upgrade lifecycle behavior.
- **MSIX:** provides Windows package identity and package-managed lifecycle,
  but requires a separate design for app data, process behavior, and
  background-daemon integration. It is not part of this proposal.
- **Per-machine MSI:** could install shared binaries and services, but would
  require administrator privileges and a different ownership/security model.
  It conflicts with Talvik's current per-user installation boundary.

## Consequences

The Windows release pipeline must construct and validate an MSI in addition
to its current ZIP artifact. MSI packaging does not itself qualify Windows
inference or runtime recovery; those requirements remain in WP-0079.

The MSI must handle version identity and upgrades without removing
`%USERPROFILE%\.marina\models`, configuration, state, or logs. Release signing
must use protected CI credentials or a separate trusted signing process.

## Implementation work package

WP-0094 SHALL define the MSI authoring/runtime prerequisites, add MSI
construction to the Windows release pipeline, and verify per-user install,
upgrade, repair, uninstall, data preservation, DLL loading, checksums, and
signing evidence.
