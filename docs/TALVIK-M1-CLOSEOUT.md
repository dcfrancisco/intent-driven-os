# Talvik M1 development closeout

Date: 2026-10-23
Baseline: `1ade5f12b792b839f0f6ffbf47a911f5e8f115ba`

## Decision

Talvik M1 development scope is **CLOSED**. The macOS x86_64 CPU-first
qualification gate is **GO** based on the recorded real-GGUF and OpenAI-client
evidence. Cross-platform product release qualification is **NOT COMPLETE**.

## Closed implementation scope

- Native llama.cpp GGUF load, generation, streaming, cancellation, embeddings,
  and tool-capability handling.
- Multiple resident models with explicit selection, unload, bounded memory
  admission/recovery, FIFO waiting, LRU eviction, pinning, and reload.
- OpenAI-compatible models, chat/completions, SSE termination metadata,
  embeddings, authentication, and supported tool-result round trips.
- Resumable acquisition, SHA-256 validation, CLI JSON/status behavior, and
  initial packaging/Docker artifacts.

## Qualification state retained independently

- macOS x86_64: GO for the recorded CPU-first release slice.
- Linux x86_64, macOS arm64, Windows x86_64: NOT EXECUTED on the current
  branch/host; native GGUF qualification remains open in WP-0078/WP-0079/
  WP-0080.
- Docker runtime: BLOCKED where the Docker daemon is unavailable; Compose
  configuration is validated.
- Continue/Cline and Codex real-client workflows: DEFERRED or NOT VERIFIED;
  no simulated client result is promoted to PASS.
- HTTP interrupted-transfer fixture: environment-blocked by local socket bind
  permission; the fixture remains an open verification obligation.

Historical records remain unchanged. Remaining obligations transfer to the
M2 work packages and the RC/platform qualification packages rather than being
silently counted as M1 passes.
