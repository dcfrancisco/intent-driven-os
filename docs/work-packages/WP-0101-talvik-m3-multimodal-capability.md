# WP-0101: Talvik M3 Multimodal Capability

## Status

In progress. Local image understanding is implemented through the pinned
llama.cpp `mtmd` adapter, with explicit model/projector pairing and OpenAI and
Ollama inline-image translation. A real image-dependent inference, cancellation,
and recovery test passed on Windows. Linux and macOS native qualification and
external OpenAI/Ollama client qualification remain open. This package does not
claim audio, video, or image-generation support.

## Purpose

Unblock local Vision clients with a backend-neutral image-input contract and a
real native inference path, while preserving Talvik's model ownership,
capability, privacy, cancellation, and recovery boundaries.

## Scope

- Represent ordered text and image content in the canonical generation
  contract without exposing native or HTTP-specific types.
- Load a vision-capable text GGUF with its explicitly registered `mmproj`
  projector artifact as one logical model.
- Integrate the pinned llama.cpp `mtmd` APIs behind the adapter boundary.
- Define and enforce limits for inline image size, decoded dimensions,
  per-request image count, and supported formats.
- Translate inline OpenAI Chat Completions image content and Ollama `images`
  into the canonical contract; reject remote image URLs.
- Preserve request cancellation, runtime policy, admission, and recovery.
- Qualify a real image-dependent answer on every supported native platform.

## Out of scope

- Audio, video, image generation/editing, remote image fetching, hosted
  providers, and implicit projector/model downloads.
- Persisting image bytes, caching them across requests, or logging image
  contents.
- Claiming all vision models, projector formats, or compatibility options are
  interchangeable.

## Implementation sequence

1. Define paired model/projector metadata and the backend-neutral ordered
   content-part contract; reject malformed and over-limit image requests. Done.
2. Package the pinned native `mtmd` library and implement image preprocessing,
   encoding, evaluation, and cleanup inside the llama.cpp adapter. Implemented;
   Windows native qualification passed.
3. Wire OpenAI inline `image_url` and Ollama `images` translation through
   existing runtime generation and cancellation paths. Implemented and covered
   by parser/IPC tests; external client qualification remains open.
4. Add contract, parser, native lifecycle, cancellation, and recovery tests;
   qualify against a pinned local vision model and deterministic image fixture.
   Windows qualification passed; Linux and macOS remain open.
5. Update the capability matrix and compatibility guides only with measured
   evidence; keep unsupported image/model combinations explicit. In progress.

## Current evidence

- `cargo test --workspace --no-run` passed with the native llama.cpp and MTMD
  libraries enabled.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- Focused model-runner, runtime, llama.cpp adapter, `talvikd`, and `talvik`
  tests passed. `talvikd` ran 35 tests; the adapter ran 4 tests with 1 ignored.
- `cargo test -p oid-console --bin oid-console` ran 29 tests: 28 passed and one
  existing shell pipeline test failed because its configured executable is
  unavailable on this Windows host (OS error 3).
- The ignored native test
  `vision_input_uses_image_content_and_recovers_after_cancellation` passed on
  Windows in 231.51 seconds. It loaded SmolVLM-256M-Instruct-f16 with its
  matching projector, identified the red square in a deterministic 64x64 PNG,
  cancelled a subsequent image generation, and then identified the square
  again.
- The current image boundary accepts up to four PNG, JPEG, BMP, or GIF images
  per request, with at most 10 MiB of encoded image bytes each, an 8192-pixel
  maximum dimension, and a 16-megapixel aggregate decoded-pixel limit.
- Qualified artifacts:
  - `SmolVLM-256M-Instruct-f16.gguf`, SHA-256
    `2f0cf33ce293f1a17b094e13e57f7ebd660ad6c0467a5e567a753e333a0c2429`
  - `mmproj-SmolVLM-256M-Instruct-f16.gguf`, SHA-256
    `0802360aca1748f112ea510b8ff277c65b1361c8ef30ed89b83c9c7a60d08e96`
- Linux/macOS native builds and image inference, third-party client workflows,
  and the supported-platform matrix are not yet qualified. Do not describe
  cross-platform vision support as complete.

## Acceptance criteria

- A text-only model reports no vision capability and rejects image requests
  before native inference.
- A vision model loads only when both the text GGUF and its declared projector
  are available and compatible; failure leaves the model unloaded and
  observable.
- Ordered text/image parts reach the backend unchanged in meaning, and a real
  image-dependent prompt produces a validated non-empty response.
- Invalid encoding, unsupported media, image count/byte/pixel limits,
  malformed inline data, and remote URLs fail with structured errors.
- Cancellation during image encoding or generation releases request-owned
  image/native resources and a subsequent request succeeds.
- Image bytes are not persisted or logged; authentication, local-only
  defaults, request limits, and OID policy remain intact.
- Native qualification records the exact model and projector revisions,
  fixture, platform, and test evidence. A placeholder or mock response does not
  satisfy the gate.

## Dependencies

ADR-0033; ADR-0003, ADR-0013, ADR-0015, ADR-0021, ADR-0025, ADR-0031, ADR-0032;
WP-0060, WP-0070, WP-0071, WP-0108, WP-0109; pinned llama.cpp `mtmd` native
builds and a locally available vision model/projector pair.
