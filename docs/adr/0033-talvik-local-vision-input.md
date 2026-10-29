# ADR-0033: Talvik Local Vision Input

- Status: Accepted
- Date: 2026-10-29
- Related: ADR-0003, ADR-0013, ADR-0015, ADR-0025, ADR-0031, ADR-0032;
  WP-0101, WP-0108, WP-0109

## Context

Talvik currently accepts text prompts and rejects image inputs. Vision is a
required capability for local assistant and intelligent-platform clients, but
the public runtime contract must remain independent of llama.cpp types and
must not silently route private images to a remote provider.

The pinned llama.cpp revision exposes multimodal image processing through its
`mtmd` library. Vision-capable GGUF models may require a separate projector
artifact (`mmproj`) in addition to the text model. Loading a text GGUF alone
does not establish image support.

## Decision

The first vision slice SHALL implement local image understanding only. Audio,
video, image generation, remote image fetching, and automatic artifact
acquisition remain separate capabilities.

The backend-neutral request contract SHALL represent images as typed content
with encoded bytes. The compatibility edge may translate supported local
payloads, but no HTTP, Ollama, OpenAI, image-decoder, or `mtmd` type may cross
the runtime API boundary.

A vision-capable local model SHALL explicitly identify both its text GGUF and
its projector artifact. The registry and loader SHALL treat the pair as one
logical model revision, validate both paths before loading, and report vision
capability only after the backend successfully initializes the projector.
Talvik SHALL NOT infer a projector by filename, download one implicitly, or
advertise vision for a text-only model.

The llama.cpp adapter SHALL own the `mtmd` context and all image preprocessing,
encoding, and native handles. Image bytes SHALL be request-scoped and SHALL
NOT be persisted or written to logs. HTTP compatibility edges SHALL accept
bounded inline image data only; remote URLs and unsupported encodings SHALL
fail explicitly. Authentication, request-size limits, cancellation, model
admission, and local-only policy continue to apply.

The initial supported formats, image count, encoded-byte limit, and decoded
pixel limit SHALL be explicit and tested. Any model/backend or request outside
that supported profile SHALL receive a structured unsupported-capability or
invalid-request error; Talvik SHALL NOT return fabricated visual results.

## Alternatives considered

- **Require a separate vision service or remote provider:** rejected for the
  first slice because it adds a new backend and can move private images off
  device.
- **Treat every GGUF as vision-capable:** rejected because many text GGUFs
  cannot consume image embeddings and some vision models require a projector.
- **Expose llama.cpp or `mtmd` types in the runtime API:** rejected because
  callers and the runtime control plane must remain backend-neutral.
- **Fetch arbitrary image URLs at the daemon:** rejected because it expands
  the network trust boundary and creates SSRF, redirect, and data-egress risks.

## Consequences

Vision loading requires paired-artifact metadata, native `mtmd` packaging, and
model-aware admission. Text-only models remain unchanged and continue to reject
images. Compatibility clients can submit inline images only when the selected
model and backend declare and successfully load vision capability.

WP-0101 is the implementation source of truth. WP-0108 and WP-0109 track the
OpenAI and Ollama edge mappings and their independent client/platform
qualification. No vision capability is complete until a real local
vision-capable model passes image-dependent tests, cancellation, recovery, and
the supported-platform matrix.
