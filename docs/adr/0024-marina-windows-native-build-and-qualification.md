# ADR-0024: Marina Windows Native Build and Qualification Requirements

- Status: Proposed
- Date: 2026-10-22
- System: Marina Model Runner
- Related: ADR-0012, ADR-0013, ADR-0021; WP-0065, WP-0079, WP-0090
- Category: Native build / dependency packaging / release qualification

## Context

ADR-0012 and ADR-0013 already establish Marina as a user-scoped,
cross-platform runner with a Windows release layout. ADR-0021 establishes
Windows loopback transport and authorization behavior. They do not define the
native build inputs or the evidence required to distinguish a buildable
Windows artifact from a qualified Windows runtime.

The repository can currently build a Windows x86_64 release in GitHub Actions
using MSVC and a pinned llama.cpp revision. Local Windows setup guidance,
native dependency checks, and end-to-end Windows runtime evidence remain
incomplete. Native llama.cpp setup scripts currently exist for Linux and
macOS (`scripts/setup-llama-linux.sh` and `scripts/setup-llama-macos.sh`), but
there is no Windows equivalent. The Windows PowerShell script is an installer
for a package that already contains the native DLLs; it does not acquire or
build them.

## Decision

Windows native Marina builds SHALL use the `x86_64-pc-windows-msvc` Rust
target and a pinned llama.cpp revision built with a supported MSVC compiler,
linker, Windows SDK, and Microsoft C/C++ runtime. CMake and Git are required
build tools.

The native build contract SHALL make these inputs explicit:

- `LLAMA_CPP_LIB_DIR` identifies the directory containing the llama.cpp import
  library and matching native DLL payload.
- `LLAMA_CPP_REQUIRED=1` makes missing native inputs a fatal build error.
- The llama.cpp revision, toolchain, runtime-library assumptions, and CPU
  baseline are recorded with release evidence.

The platform setup boundary SHALL remain explicit: Linux and macOS may use
their native llama.cpp setup scripts, while Windows source builds currently
require manually installed MSVC/CMake/Git tooling and a manually prepared
llama.cpp native payload until a Windows setup path is delivered.

Windows release archives SHALL bundle the matching llama.cpp and ggml DLLs.
Consumers must not need Ollama, a system-wide llama.cpp installation, a
machine-wide PATH change, or administrator privileges to run the packaged
Marina binaries. Installation and transport behavior remain governed by
ADR-0012, ADR-0013, and ADR-0021.

Windows support SHALL not be marked qualified from compilation or packaging
alone. Qualification requires native Windows evidence for DLL loading, GGUF
discovery, inference, cancellation, timeout, recovery, authorization, and
the documented user-scoped installation layout.

## Scope boundaries

This ADR governs native Windows build and release evidence for Marina. It does
not redefine Marina's product boundary, installation layout, transport,
authorization model, or OID's Linux-first scope.

It does not authorize a Windows-specific backend API, GPU support, or Ollama
as a runtime dependency.

## Consequences

Windows release claims become tied to reproducible native inputs and runtime
evidence rather than compilation alone. The project must maintain compatible
MSVC/runtime and native DLL provenance records, and provide a documented
developer setup path.

The native build/readiness work and native runtime qualification remain
separate deliverables: WP-0090 covers the former, while WP-0079 covers the
latter.

## Initial implementation work package

WP-0090 SHALL define and deliver the Windows prerequisite check, reproducible
llama.cpp build path, dependency packaging checks, and build documentation.
WP-0079 SHALL provide the native Windows runtime qualification evidence.
