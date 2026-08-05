# WP-0030: Tokenizer

## Purpose

Expose backend-native token counting through the runtime without implementing
a tokenizer algorithm in the project.

## Scope

UTF-8 text and file counting routed to the loaded llama.cpp vocabulary.

## Deliverables

- Backend tokenizer contract.
- `tokenize <text>` and `count <path>` console commands.
- Tokenizer readiness events.

## Acceptance Criteria

The runtime reports deterministic errors when no model/tokenizer is loaded and
never performs text generation.

## Dependencies

WP-0027, WP-0029.

## Future Work

Batch tokenization, context admission, prompt accounting, and remote-provider
token usage normalization.
