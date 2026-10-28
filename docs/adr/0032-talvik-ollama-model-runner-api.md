# ADR-0032: Talvik Ollama-Compatible Model Runner API

- Status: Accepted
- Date: 2026-10-28
- Related: ADR-0003, ADR-0013, ADR-0021, ADR-0025, ADR-0031;
  WP-0091, WP-0096, WP-0097, WP-0108, WP-0109

## Context

At the time this ADR was written, Talvik already discovered, imported, loaded,
served, and unloaded local GGUF models and exposed authenticated `/api/tags`
and `/api/ps`, an OpenAI-shaped edge, and an internal runtime API. The initial
text and inspection routes have since been implemented, but clients still
cannot use Talvik as a complete Ollama-compatible runner. Model
lifecycle/distribution operations and request-specific runtime controls remain
incomplete.

The official Ollama API documents inference (`/api/chat`, `/api/generate`,
`/api/embed`), local model inspection and process state (`/api/show`,
`/api/tags`, `/api/ps`, `/api/version`), and model lifecycle/distribution
(`create`, `copy`, `pull`, `push`, `delete`, and blob operations). Talvik has
some corresponding internal model lifecycle primitives but does not implement
Ollama's registry/manifest/Modelfile distribution system. These differences
must be visible rather than hidden behind success-shaped routes.

## Decision

Talvik SHALL provide a documented, progressively qualified Ollama-compatible
**local model-runner API profile**. The official Ollama API reference at
`https://docs.ollama.com/api/introduction` and its linked endpoint references
are the compatibility baseline; WP-0109 SHALL record the profile inventory
and recheck it when the public API evolves.

The profile is limited to local model serving, inference, and local model
management. It does not imply compatibility with Ollama cloud accounts,
hosted-only services, or every model/backend capability. The internal Talvik
API remains backend-neutral; Ollama request and response types are translated
at the HTTP edge.

The work is phased:

1. **Text routes and client qualification:** Ollama-shaped `/api/chat` and
   `/api/generate` requests and responses, NDJSON streaming, supported
   generation options, finish metadata and measured statistics; `/api/embed`
   plus legacy `/api/embeddings`; and structured Ollama errors. Qualify the
   official Python and JavaScript clients on each supported native platform,
   including a real model/backend embedding check, before model-management
   operations begin. Text-only and one active generation remain the current
   runtime capabilities.
2. **Verified pull and safe local operations:** implement digest-verified
   model acquisition with progress, interruption recovery, storage bounds,
   explicit source trust, and safe copy/delete with active-use and pinning
   protection, durable registry updates, and artifact-ownership rules.
3. **Model definition and publishing:** Modelfile create, registry push, and
   blob distribution require separate runtime and artifact contracts and are
   not implied by the raw verified-pull slice.
4. **Capability expansion:** images, tool calls, structured output, thinking,
   and other modalities only when the selected model and backend genuinely
   support them.

Routes SHALL return the expected Ollama wire shape for implemented behavior.
Recognized unsupported options, endpoints, and modalities SHALL fail
explicitly. Unknown model metadata SHALL be omitted or represented as unknown;
Talvik SHALL NOT fabricate artifact sizes, model templates, capabilities, or
usage measurements.

When an Ollama client schema requires a metadata field that Talvik cannot
truthfully populate, Talvik SHALL preserve the required response shape with an
empty value (for example, `model_info: {}`) rather than inventing metadata.
Client qualification evidence and remaining platform gaps are tracked in
WP-0109 and the compatibility guide.

Existing authentication, authorization, loopback defaults, OID policy,
admission, cancellation, and model lifecycle invariants remain in force. Any
compatibility exception must be an explicit operator configuration, never an
implicit weakening of security.

## Alternatives considered

- **Keep only `/api/tags` and `/api/ps`:** rejected because it prevents common
  model-runner clients from performing inference through Talvik.
- **Make Ollama the internal runtime boundary:** rejected because Talvik must
  remain backend-neutral and independently governed.
- **Implement every route as an immediate stub:** rejected because model
  creation, registry distribution, blobs, and multimodal inference require
  real storage, integrity, and backend contracts.
- **Claim full compatibility from endpoint presence:** rejected; each profile
  operation must pass request/response, stream, error, lifecycle, security,
  and client qualification.

## Consequences

ADR-0025's M2 decision to add only client-justified `/api/tags` and `/api/ps`
was valid for its original M2 scope. This ADR supersedes its decision to defer
the other Ollama-compatible endpoints without an identified client need. It
does not supersede ADR-0025's scheduler, security, backend-neutrality, or
resource-governance invariants.

WP-0109 is the implementation source of truth. Current support and gaps are
maintained in `docs/TALVIK-OLLAMA-COMPATIBILITY.md` and the compatibility
capability matrix. A workflow definition alone does not qualify a platform;
each client/platform job must pass and retain its evidence. Full compatibility
is not claimed until every in-scope profile endpoint has tests and
qualification evidence.

### Qualification handoff status

At the 2026-10-28 handoff, Windows Python and JavaScript inference-client
checks and both-language embedding checks have passed. Native Linux and macOS
jobs remain pending in `.github/workflows/talvik-api-client-qualification.yml`.
Phase 2 (verified pull and safe copy/delete) must remain gated until those jobs
pass and their evidence is recorded in WP-0109.
