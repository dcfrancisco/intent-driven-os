# ADR-0019: OID Product Brand Architecture

- Status: Accepted
- Date: 2026-10-07
- Owner role: Principal Product Marketing
- Related: ADR-0011, ADR-0013, ADR-0016, ADR-0017, ADR-0018; WP-0075

## Brand architecture

The product family has distinct names because the products have distinct users,
jobs, and release surfaces:

| Brand | Product role | Primary promise | Release scope |
| --- | --- | --- | --- |
| **OID** / **Open Intelligence Desktop** | AI desktop/OS control plane | Prompt the computer; understand, plan, operate, verify, and recover | Linux-first distro/product |
| **Marina** | Standalone model runner and local AI service | Run and expose models reliably on a developer machine | Linux, macOS, Windows |
| **OID OS Model** | Specialized model for computer management | Turn computer context into safe typed plans and proposals | Served through Marina |
| **Rust CA-Clipper subsystem** | Linux business/application subsystem | Bring the CA-Clipper domain forward into the OID Linux product | Linux OID distro |

Marina is not the name of the operating system. OID is not the name of the
model runner. The OS-management model is not the agent, policy engine, or
operating system. This separation must appear in documentation, installers,
UI labels, API metadata, screenshots, and launch messaging.

## Positioning

OID: “An AI desktop that makes the computer understandable, operable, and
recoverable—with the user in control.”

Marina: “A local-first model runner for developers, assistants, and OID.”

The product story starts with a working local model, but the differentiated
value is governed computer agency: context, plans, approvals, bounded skills,
verification, recovery, and evidence. Claims must distinguish implemented,
preview, planned, and unavailable capabilities.

## Naming and messaging rules

- Use **Marina model runner** or **Marina service** for the standalone runner.
- Use **OID AI desktop/OS** for the Linux product.
- Use **OID OS-management model** until a model name is deliberately approved.
- Do not call Marina an operating system, autonomous agent, or full gateway
  until those capabilities are implemented and validated.
- Do not describe OID as cross-platform merely because Marina is.
- Describe self-management as policy-bounded and verifiable, never as
  unrestricted autonomy.
- Product pages and CLI help must identify the active model, backend, and
  implementation status where relevant.

## Launch sequence

1. Marina local model runner: install, discover, load, chat, stream, cancel,
   inspect, and authenticate clients.
2. Marina assistant edge: stable OpenAI/Ollama-compatible subset and IDE use.
3. OID Linux AI desktop: governed plans, skills, approvals, verification, and
   recovery.
4. OID OS-management model: specialized model evaluation and release.
5. OID Linux distro: integrated Marina, OID, model variants, and Linux
   subsystems such as Rust CA-Clipper.

This ADR governs product language and portfolio boundaries; it does not claim
that the later products or capabilities are implemented.
