# Marina Principal Architecture Audit

- Date: 2026-10-07
- Scope: Linux distribution product, standalone model runner, AI service,
  OID integration, and Ollama/ Docker Model Runner/OmniRoute capability parity
- Result: direction is sound; Linux-ready and AI-service-ready status are not
  yet earned

## Executive finding

Marina has a credible local GGUF runner slice: model discovery/import, one
active model, llama.cpp loading, streaming, cancellation, and basic metrics.
It is not yet a Linux distribution product or a general AI gateway. The prior
architecture described the desired service but did not define enough executable
contracts for installation profiles, service ownership, transport trust,
package provenance, recovery, or AI capability semantics.

## Critical gaps

| Area | Current truth | Required decision/work |
| --- | --- | --- |
| Installation | Development shell installer; no packages or service units | ADR-0014, WP-0069: user/system profiles, FHS paths, systemd, packages |
| Transport | Local tab-delimited Unix socket; no peer authorization or API version | ADR-0014/0015, WP-0066: authenticated framed/versioned transport |
| Service | Foreground daemon; no lifecycle commands, restart policy, or stale-socket recovery | WP-0069 and WP-0062: supervisor, recovery, migration, rollback |
| Trust | Optional checksum HTTP import; no release/backend/model signatures | ADR-0014, WP-0058/WP-0071: separate artifact trust and provenance |
| Linux | No distro support matrix, rootless/system semantics, GPU permission policy, or sandbox profiles | ADR-0014, WP-0069/WP-0071 |
| AI API | Text generation contract only; no canonical chat/session/provider schema | ADR-0015, WP-0070 |
| Compatibility | No OpenAI/Ollama/Anthropic HTTP edge | WP-0067 and WP-0070 |
| Routing | No production multi-backend router, fallback, quota, cost, or circuit breaker | WP-0060, WP-0071 |
| Assistants | Tool execution/session/evidence boundary is not implemented | WP-0063, WP-0068, WP-0070/0071 |
| Evaluation | No conformance/quality/latency/cost/tool-safety release suite | WP-0070/0071 |

## Linux product stance

The initial supported distro baseline is Debian/Ubuntu and Fedora/RHEL-family
x86_64 CPU installations with systemd. ARM64 and accelerator variants require
separate evidence. A successful compilation is not sufficient for a support
claim. User services and system services have distinct identities, paths,
sockets, locks, registries, and model ownership.

The portable user experience remains `$HOME/.marina`, including models under
`$HOME/.marina/models`. Distro packages use FHS-aligned executable/config/state
paths while preserving explicit model ownership. No system service reads a
user home by accident, and no local socket is treated as authorization by
itself.

## AI product stance

Marina should combine:

- Ollama-like local model management and chat/generation ergonomics;
- Docker Model Runner-like protocol compatibility, engine abstraction, and
  model packaging/on-demand execution;
- OmniRoute-like provider routing, policies, fallbacks, quotas, circuit
  breakers, route explanations, and MCP/A2A control surfaces.

The internal contract remains Marina-owned. Compatibility protocols translate
to it and publish capability gaps. Tool calls remain proposals until a governed
OID/system host authorizes and verifies execution.

## Sources reviewed

- Repository: `AGENTS.md`, `ARCHITECTURE.md`, `runtime/ARCHITECTURE.md`,
  ADR-0011 through ADR-0015, runtime/ROADMAP.md, runtime/BACKLOG.md, and
  WP-0057 through WP-0071.
- Runtime: installer, IPC, configuration, model acquisition/discovery,
  service composition, hardware detection, and llama.cpp build boundary.
- External references: Ollama API documentation, Docker Model Runner API and
  gateway documentation, OmniRoute user guide, Linux FHS, and systemd service,
  user-unit, credential, and sandbox documentation.

## Handoff

Outcome: Principal Linux distro and AI architecture audit completed; ADR gaps
promoted into explicit decisions and bounded work packages.

Files changed: ADR-0012, ADR-0013, new ADR-0014, new ADR-0015, new WP-0069,
WP-0070, WP-0071, runtime architecture/roadmap/backlog, and this audit.

Evidence/tests: Source audit, official Linux/systemd references, official
Ollama and Docker Model Runner API references, OmniRoute user guide, `cargo
fmt --all -- --check`, `cargo check --workspace`, `cargo test -p oid-runtime`,
and `git diff --check`.

Remaining risks: Packaging, service supervision, authenticated transport,
artifact trust, resource isolation, compatibility APIs, and AI evaluation are
still implementation gaps.

Next dependency: finalize WP-0069 installation profiles and systemd units,
then implement WP-0066 transport identity and WP-0070 canonical AI contracts
before claiming Linux-ready or assistant-ready status.
