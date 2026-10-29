# WP-0110: Talvik Gemma 4 Default Model

## Status

In progress. The pinned Gemma 4 E4B-it Q4_0 GGUF and Q8_0 vision projector are
configured as Talvik's automatic default and have been downloaded and
checksum-verified on Windows. The optimized daemon completed bootstrap and
projector pairing, but memory admission blocked loading on the test host, so
text, image, cancellation, and recovery inference remain unqualified. Linux
and macOS native qualification remain separate open gates.

## Purpose

Make Talvik useful immediately after installation with a default local model
that supports both text and the implemented image-input path, while retaining
verified artifacts, explicit model/projector pairing, user overrides, and
startup failure visibility.

## Scope

- Set Gemma 4 E4B-it Q4_0 as the built-in default and pin the upstream revision,
  artifact URLs, and SHA-256 values.
- Automatically acquire and verify the model and projector into the user's
  model store, pair the projector before loading, and avoid registering it as a
  runnable model.
- Preserve `TALVIK_DEFAULT_MODEL`, model source/checksum overrides,
  auto-download/load controls, and `default_model: none`.
- Update onboarding and packaged model metadata to state the first-run download
  size and resource implications.
- Qualify real text and image inference, cancellation, recovery, and startup
  readiness with the pinned artifacts.

## Acceptance criteria

- A fresh default bootstrap downloads both pinned files, verifies each SHA-256,
  persists the projector pairing, loads the model, and marks Talvik ready.
- The projector is not exposed as an independent runnable model.
- A failed download, checksum, pairing, or load produces a warning and leaves
  readiness false; no fallback model is loaded silently.
- A configured alternate model or `default_model: none` continues to work.
- The pinned model answers text requests and image-dependent requests, and
  recovers after cancellation.
- Windows, Linux, and macOS qualification results are recorded independently;
  no platform is described as qualified without a real native run.

## Artifact pin

- Model: `ggml-org/gemma-4-E4B-it-GGUF`, revision
  `a69f018d7678a2b9f5e705f8903d6fb29b9b25dc`, Q4_0,
  SHA-256 `a555b900214b477d8880e7832e0b8925e139b0159640036b09fe472b6f2097f2`.
- Projector: same repository and revision, Q8_0,
  SHA-256 `197f49a93027f9843772bd24a6a9e0be2a32a788de5a3def330e9c585d86edd1`.
- Combined first download: 5,150,682,208 bytes (about 4.8 GiB).

## Qualification record

- Windows, 2026-10-29: both downloaded artifacts matched their pinned sizes
  and SHA-256 values. The release `talvikd` completed default bootstrap and
  projector pairing, then rejected model loading through resource admission:
  3,570,778,112 bytes available versus 5,755,633,664 bytes required. No
  inference was run; repeat text, image, cancellation, and recovery tests when
  sufficient memory is available.
- Linux and macOS: not yet tested.

## Dependencies

ADR-0034; ADR-0033; WP-0101; pinned llama.cpp Gemma 4 and `mtmd` support.
