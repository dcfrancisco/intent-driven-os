# WP-0031: Inference Engine

## Purpose

Define the first backend-neutral end-to-end generation contract.

## Scope

Independent prompts, sampling options, context size, results, statistics, and
runtime-owned streams. No conversation memory or prompt templates.

## Deliverables

- `GenerationRequest` and `GenerationOptions`.
- `GenerationStream`, `GenerationResult`, and `GenerationStatistics`.
- Backend callback boundary for token pieces.

## Acceptance Criteria

Generation is represented without exposing llama.cpp types to clients.

## Dependencies

WP-0027, WP-0029, WP-0030; ADR-0003, ADR-0004.

## Future Work

Additional backends, context policies, and richer sampling controls.
