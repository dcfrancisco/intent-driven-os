# WP-0090: Marina Windows Support Readiness

## Purpose

Make the proposed Windows support boundary operational for Marina without
coupling OID to Windows. This work package covers reproducible native build
inputs, local prerequisite guidance, self-contained release packaging, and the
documentation needed before Windows support can be claimed.

The current repository has native llama.cpp setup scripts for Linux and
macOS, but no Windows equivalent. `scripts/install-model-runner.ps1` installs
only a packaged Windows archive and does not download or build native DLLs.

## Scope

- Define supported Rust, MSVC, Windows SDK, Microsoft C/C++ runtime, CMake,
  Git, and llama.cpp versions or compatibility ranges.
- Provide a Windows-native prerequisite and toolchain check for developers and
  release operators.
- Add the Windows equivalent of the existing Linux/macOS native llama.cpp
  setup path, or document an approved replacement that produces the required
  `.lib` and `.dll` payload.
- Provide a reproducible PowerShell path for building the pinned llama.cpp
  revision and placing its `.lib` and `.dll` outputs in `LLAMA_CPP_LIB_DIR`.
- Verify that Windows builds fail clearly when required native inputs are
  missing or mismatched.
- Verify that release archives contain the required Marina executables and
  native DLL payload, and that installation keeps the payload user-scoped and
  loader-visible.
- Document local source builds, packaged installs, `%USERPROFILE%\\.marina`
  paths, `LLAMA_CPP_LIB_DIR`, and the Windows transport defaults.
- Coordinate with WP-0079 for native runtime qualification; do not duplicate
  its inference and recovery acceptance tests.

## Out of scope

- Windows OID desktop or operating-system support.
- A new Windows-specific model backend or public backend-native API.
- Reference local runner as a runtime dependency.
- GPU or accelerator qualification beyond the CPU-first release target.
- Declaring Windows runtime support qualified without WP-0079 evidence.

## Acceptance criteria

- A documented Windows environment can check prerequisites before building.
- The documentation clearly distinguishes the Linux/macOS setup scripts from
  the Windows packaged-release installer and source-build requirements.
- The pinned llama.cpp revision builds with the supported MSVC toolchain and
  produces the import library and DLL set required by Marina.
- A source build with `LLAMA_CPP_REQUIRED=1` succeeds when the native payload
  is present and fails with an actionable error when it is absent.
- A packaged Windows archive contains `talvikd.exe`, `talvik.exe`, and the
  matching native DLL payload; installation requires no administrator rights
  or global PATH mutation.
- The installed binaries can locate their bundled native DLLs from the
  documented user-scoped layout.
- Documentation distinguishes artifact construction from native runtime
  qualification and links to WP-0079.
- Release-readiness records identify the exact toolchain, native revision,
  runtime-library assumptions, and remaining qualification status.

## Dependencies

ADR-0024, ADR-0012, ADR-0013, ADR-0021, WP-0065, WP-0066, WP-0077, and
WP-0079. A Windows x86_64 host or GitHub Windows runner is required for
end-to-end validation; the current repository records that qualification as
blocked until such execution is available.

## Deliverables

- Windows native prerequisite/build setup documentation and PowerShell
  tooling.
- Dependency and archive validation checks.
- Updated Marina Windows installation/build documentation.
- A release-readiness update linking build evidence to WP-0079 runtime
  qualification evidence.

## Status

Proposed. No implementation is authorized by this planning record alone.
