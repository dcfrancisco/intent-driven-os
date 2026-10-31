# WP-0060: Backend Capability and Router

## Purpose

Add an OID-owned, deterministic router that can choose a backend without
coupling callers to llama.cpp, Reference local runner, or a remote provider.

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
- Routing can be disabled globally; disabled is the default.
- Direct model selection remains available when routing is enabled.
- Fallback is explicit and observable, and is suppressed after any streamed
  output to avoid combining responses from different models.

## Implemented first slice — 2026-10-17

The primary `talvik` CLI implements opt-in, ordered fallback among local model
IDs through the existing public runtime protocol. It records route decisions
to the invoking terminal and falls through only on load failure or generation
failure before output. This is a bounded operational slice, not completion of
the backend capability router. Persistent route evidence, policy snapshots,
cross-backend selection, remote providers, health scoring, and circuit
breaking remain open in this WP.

## Dependencies

WP-0023, WP-0058, WP-0059; ADR-0003, ADR-0007, ADR-0011.

## Loaded profile and output-format slice — 2026-10-31

Implemented an immutable backend-neutral loaded-model profile, populated from
native artifact/tokenizer/template/decoder/projector observations. Native
profiles are cached separately from the inference mutex and removed on unload;
API discovery remains read-only and reports unknown until actual load. Existing
backend implementations can return unknown without inventing support.

Native cache policy follows observed recurrent/hybrid versus attention support.
Text generation respects tokenizer-declared BOS insertion exactly once; public
literal tokenization is unchanged. Template-selected JSON and function/parameter
tool protocols drive both generation instructions and streaming/nonstream
parsing. Leading reasoning is split before tools, with separate chat transport
fields and explicit supported-control checks. Unsupported tool formats and
unimplemented pooled embedding execution are not promoted from declarations.

Unit fixtures cover schema-preserving function parameters, duplicate/malformed
payloads, arbitrary chunk boundaries, Unicode, reasoning/tool isolation and
unsupported declarations. Native fixtures cover profile lookup while the
inference lock is held, unload invalidation and metadata-driven BOS behavior.
Live acceptance is `scripts/test-talvik-model-profiles.py`. This slice does not
implement automatic multi-provider routing, complete arbitrary Jinja rendering,
pooled embedding execution or cross-model admitted cache retention.
