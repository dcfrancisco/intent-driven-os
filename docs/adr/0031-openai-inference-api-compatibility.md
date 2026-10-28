# ADR-0031: OpenAI Inference API Compatibility

- Status: Accepted
- Date: 2026-10-28
- System: Talvik Model Runtime
- Related: ADR-0013, ADR-0015, ADR-0021, ADR-0025; WP-0067, WP-0092,
  WP-0108

## Context

Talvik exposes a local OpenAI-shaped HTTP edge, currently centered on text
`/v1/chat/completions`. The route works with a basic OpenAI client, but the
current implementation is not fully wire- or behavior-compatible: some
declared routes have the wrong response shape, unsupported request options can
be ignored, errors are not consistently structured, and current model
capabilities do not include every OpenAI modality.

OpenAI's API surface evolves independently of Talvik. “Full compatibility”
must therefore identify a dated contract profile, separate endpoint
compatibility from model capability, and remain a translation edge rather
than becoming Talvik's internal runtime contract.

## Decision

Talvik SHALL implement a versioned compatibility profile for the OpenAI
**model-serving and inference API surface**, using the OpenAI public API
reference available on 2026-10-28 as the initial baseline. The profile covers
model discovery and inference operations, including text completions, chat
completions, Responses, embeddings, and supported image/audio/multimodal
operations. WP-0108 SHALL pin the exact endpoint and schema inventory and
record later additions as profile revisions.

The profile does not include OpenAI account, organization, project, billing,
fine-tuning, evaluation-management, or other platform-administration APIs.
Talvik is a local model-serving runtime and SHALL NOT create success-shaped
stubs for those unrelated services.

Compatibility SHALL follow these principles:

- The OpenAI protocol remains an edge translation. Backend-neutral Talvik
  request, response, event, capability, and error types remain canonical.
- Every implemented endpoint SHALL match its selected request, response,
  error, and streaming wire shapes. Legacy endpoints SHALL either be
  implemented correctly or return an explicit unsupported-endpoint response;
  they SHALL NOT alias to a different response schema.
- Standard request parameters SHALL be honored according to the profile or
  rejected with a structured, OpenAI-shaped error. Silent fallback to a
  default for invalid values and silent ignoring of recognized unsupported
  options are not acceptable.
- Model and backend capability SHALL be explicit. A protocol endpoint may
  return a structured unsupported-capability error when the selected model or
  backend cannot perform the requested modality or operation. Talvik SHALL
  NOT fabricate image, audio, tool, or structured-output results.
- Responses and SSE streams SHALL include required profile metadata, usage
  when available or requested, correct terminal events, and stable request
  correlation. Talvik-specific diagnostics remain additive and documented.
- Authentication, local-only defaults, authorization scopes, cancellation,
  admission, and OID policy boundaries remain authoritative; compatibility
  SHALL NOT weaken them.
- Conformance claims SHALL be limited to endpoints and capabilities with
  automated contract tests and recorded client qualification.

## Alternatives considered

- **Keep the present subset and document limitations only:** accurate but does
  not satisfy the product requirement for broad OpenAI-client interoperability.
- **Make OpenAI types the internal Talvik API:** rejected because it couples
  runtime policy and backend-neutral contracts to a third-party protocol.
- **Return placeholders for unsupported modalities:** rejected because
  success-shaped behavior would mislead clients and conceal missing model
  capabilities.
- **Implement every OpenAI platform endpoint:** rejected for the initial
  profile because account, billing, project, and training-management APIs are
  not responsibilities of a local inference runtime.

## Consequences

The current compatibility guide remains the snapshot of implemented
behavior, while WP-0108 becomes the authoritative implementation and
qualification plan. The existing text edge must be corrected before its
compatibility claim is broadened. Responses and multimodal routes require
backend-neutral capability and request/response contracts, plus backend
implementations that can actually serve those modalities.

No endpoint is considered fully compatible merely because it returns HTTP
200 or because one standard client completes a smoke request.

### Qualification handoff status

At the 2026-10-28 handoff, the documented OpenAI text-profile smoke checks and
embedding SDK checks have passed on Windows with Python and JavaScript clients.
Native Linux and macOS results remain pending in
`.github/workflows/talvik-api-client-qualification.yml`. This checkpoint does
not change the accepted profile or establish full OpenAI compatibility; WP-0108
tracks the remaining evidence and implementation gaps.
