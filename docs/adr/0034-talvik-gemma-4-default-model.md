# ADR-0034: Gemma 4 E4B-it as Talvik's Default Model

- Status: Accepted
- Date: 2026-10-29
- Related: ADR-0033, WP-0101, WP-0110

## Context

Talvik's built-in default was a small text-only model. That made first-run
inference lightweight, but did not exercise the local image-input capability
or provide a useful default for assistant and vision clients.

The pinned llama.cpp revision includes Gemma 4 model support. Google's
Gemma 4 E4B-it is Apache-2.0 licensed and accepts text and image input. The
`ggml-org` Hugging Face repository provides a GGUF quantization and a separate
vision projector compatible with the native `mtmd` path.

## Decision

Talvik SHALL default to `gemma-4-e4b-it-q4_0`, using the Q4_0 GGUF and Q8_0
vision projector pinned to revision
`a69f018d7678a2b9f5e705f8903d6fb29b9b25dc` in
`ggml-org/gemma-4-E4B-it-GGUF`. Both artifacts SHALL be verified against their
recorded SHA-256 values before use.

When either artifact is missing and automatic download is enabled,
`talvikd` SHALL acquire it into the user model directory, pair the projector
with the registered model, and only then load the model. The projector is an
artifact, not a separately registered runnable model. A failed transfer,
checksum, pairing, or load SHALL be reported and SHALL leave readiness false;
the daemon SHALL NOT silently load another model.

Users may select another default through the existing `TALVIK_DEFAULT_MODEL`,
source, and checksum settings, configure a projector source when needed, or
disable default loading with `TALVIK_DEFAULT_MODEL=none`. Existing model files
are not deleted or migrated.

## Consequences

- A first-time install downloads about 5.15 GB (4.8 GiB) for the model and
  projector. The daemon needs sufficient disk space and memory; CPU-first
  inference may be slower than the previous small-model default.
- The default exercises local image input. Talvik still does not claim audio,
  video, or image-generation support, even though Gemma 4 has additional model
  capabilities.
- The selected model, projector, and native path must be qualified on each
  supported platform. Windows real-model qualification is tracked in WP-0110;
  Linux and macOS remain open until tested.
