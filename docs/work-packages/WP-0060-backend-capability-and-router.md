# WP-0060: Backend Capability and Router

## Purpose

Add an OID-owned, deterministic router that can choose a backend without
coupling callers to llama.cpp, Ollama, or a remote provider.

## Scope

Capability negotiation, candidate filtering, local-first policy, health and
capacity inputs, fallback rules, circuit breaking, and explainable routing
decisions. No autonomous agent planning.

## Acceptance criteria

- The same inputs produce the same route and decision explanation.
- Unsupported context, streaming, tool, modality, or structured-output needs
  are rejected before backend execution.
- A backend failure can trigger only policy-allowed fallback and records the
  changed backend explicitly.
- Existing llama.cpp behavior remains the reference local path.

## Dependencies

WP-0023, WP-0058, WP-0059; ADR-0003, ADR-0007, ADR-0011.
