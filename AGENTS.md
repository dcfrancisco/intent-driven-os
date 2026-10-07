# OID assistant and multi-agent instructions

This file is the durable briefing for AI assistants working in this repository.
Read it before acting. Do not ask the user to restate this project context when
the task is covered here.

## Mission

OID (Open Intelligence Desktop) is a Linux-first, Rust-based AI desktop/OS and
keyboard-first console. Marina is the cross-platform standalone model-runner
service for Linux, macOS, and Windows; OID is one client of its public API.
The product goal is a computer users can prompt to inspect, plan, operate,
verify, recover, and maintain itself under explicit policy and human approval
boundaries.
The runtime owns model lifecycle, policy, hardware awareness, streaming,
cancellation, observability, and recoverability. The console is a client and
must not own runtime state or backend-native handles.

The near-term product proof is a trustworthy local model service: load a local
model, run an independently addressable streaming request, cancel it, observe
health/metrics, and expose a small compatibility API. The long-term product is
an OID-owned router that can select local or remote backends by capability,
policy, health, and resource fit.

## Current truth

- The workspace builds a native llama.cpp boundary behind `LLAMA_CPP_LIB_DIR`.
- GGUF discovery, one-model load/unload, tokenization, streaming generation,
  cancellation, and basic generation metrics exist in code.
- The current runner is CPU-first and single-model/single-generation; it is not
  yet a production scheduler, model downloader, chat-history system, tool loop,
  or multi-backend router.
- The governed operation lifecycle (plan, approval, execution, verification,
  rollback, evidence) is a separate foundation and must remain intact.
- Ollama is an interoperability target and reference UX, not OID's internal
  architectural boundary. OID must not require Ollama to run local models.
- The model runner is independently installable: `marina` and `marinactl` are
  the service/client product surface; `oid-console` is an optional client.
- The canonical user installation is `$HOME/.marina` on Linux/macOS and the
  equivalent per-user Marina home on Windows; binaries, native libraries, and
  models do not belong in the source repository.
- Marina must expose a versioned API usable by OID, AI assistants, and other
  system clients; OpenAI-compatible HTTP is an edge, not the internal API.
- Only Marina is a cross-platform release for Linux, macOS, and Windows. OID
  and the AI desktop/OS are Linux-first; a dedicated OID model may be served
  by Marina but must never bypass OID policy, approval, evidence, or skills.

## Working rules

1. Inspect the repository and relevant ADR/WP before changing architecture.
2. Prefer small, reviewable slices with tests and documentation updated in the
   same change.
3. Preserve backend neutrality: no llama.cpp, Ollama, provider SDK, tensor, or
   native pointer types may cross the runtime API.
4. Preserve security and recoverability: model artifacts, requests, routing
   choices, approvals, failures, and lifecycle transitions need explicit policy
   and observable evidence.
5. Do not broaden scope silently. If a request implies a new architectural
   decision, add or update an ADR and link the implementing WP.
6. Do not claim a feature is complete because a placeholder compiles. State
   whether it is implemented, contract-only, unavailable without a native
   dependency, or intentionally deferred.
7. Run the narrowest relevant checks first, then workspace checks when practical:
   `cargo fmt --all -- --check`, `cargo test --workspace`, and strict Clippy.

## Multi-agent protocol

Agents may work in parallel only when their write sets are disjoint. Use these
lanes:

- **Architecture/docs:** ADRs, roadmap, WPs, `AGENTS.md`, and inventory.
- **Runner:** `crates/model-runner`, `runtime/adapters/llama-cpp`, and native
  FFI safety/tests.
- **Runtime control plane:** `runtime/src`, model registry, admission, routing,
  API, lifecycle, and persistence.
- **Console/client:** `console/`, command grammar, transport, and rendering.
- **Verification:** tests, benchmarks, Linux validation, and compatibility
  fixtures.

Every agent must report: files changed, decisions made, tests run, known gaps,
and the next dependency. The coordinating agent integrates overlapping changes,
updates the ADR/WP status, and is the only agent that declares a milestone done.
Never have two agents edit the same file concurrently.

## Required handoff format

```text
Outcome:
Files changed:
Evidence/tests:
Decisions or assumptions:
Remaining risks:
Next dependency:
```

## Canonical references

- Project brief: `docs/PROJECT-OPERATING-BRIEF.md`
- Architecture: `ARCHITECTURE.md`, `runtime/ARCHITECTURE.md`
- Decisions: `docs/adr/README.md`
- Work packages: `docs/work-packages/README.md`
- Runtime plan: `runtime/ROADMAP.md`, `runtime/BACKLOG.md`
