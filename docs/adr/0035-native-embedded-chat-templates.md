# ADR-0035: Execute Embedded Chat Templates

- Status: Accepted
- Date: 2026-11-01
- Related: ADR-0025, ADR-0026, ADR-0031, ADR-0033; WP-0098

## Context

The llama.cpp C role/content formatter selects built-in layouts. It does not
execute the GGUF Jinja tools branch. Synthesized tool examples, approximate
system ordering, and prose versions of tool results produced misleading
Agent behavior. A completed HTTP response did not establish command execution.

## Decision

The native adapter executes the artifact's actual embedded Jinja template with
the engine from the pinned llama.cpp source. A private `oid-chat-template`
library accepts UTF-8 template/context data and returns owned UTF-8 data through
a versioned C ABI. Native allocations are freed by that library. No native
pointers, template engine types or provider objects cross the runtime API.

Messages, tool definitions, historical arguments and correlated tool results
remain structured until template execution. No tool name, project command or
model identifier is hard-coded in the backend formatter. Plain chat can retain
the built-in C fallback when the embedded renderer is unavailable; tool support
requires a usable renderer, a successful tool-template probe and a supported
output parser. Missing support is reported instead of approximated silently.

Recurrent cache boundaries are derived by rendering a structurally identical
conversation with a unique last-user marker, retaining only a verified prefix
of the real output. Existing token-prefix and native-state checks still apply.
Multimodal requests retain their existing cache/admission restrictions.

Setup scripts build the renderer against the same pinned source and ggml-base
library. Packages include the additional library. Linux and Windows changes
require platform qualification; macOS validation is not cross-platform proof.

The runner emits calls; clients execute them under their own approvals. Native
template fidelity does not guarantee arbitrary model instruction-following.
Live IDE execution, scope compliance and API conformance are separate gates.
