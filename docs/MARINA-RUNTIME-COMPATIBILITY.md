# Marina Runtime Compatibility Matrix

This matrix records verified combinations. An empty or unverified cell is not
evidence of support.

| OS | Architecture | Backend | Format | Quantization | Transport | Status |
| --- | --- | --- | --- | --- | --- | --- |
| macOS | x86_64 | llama.cpp | GGUF | Q4_K_M | Unix socket + loopback HTTP | Verified locally |
| macOS | arm64 | llama.cpp | GGUF | Q4_K_M | Unix socket + loopback HTTP | Not yet verified |
| Linux | x86_64 | llama.cpp | GGUF | Q4_K_M | Unix socket + loopback HTTP | Not yet verified |
| Windows | x86_64 | llama.cpp | GGUF | Q4_K_M | Loopback TCP + HTTP | Not yet verified |

## Current verification notes

- The macOS x86_64 row has passed native GGUF loading and real HTTP
  completion through the Rust server.
- The same row has now passed a PTY-backed OID console session in explicit
  `marina-http` mode, including cancellation followed by a successful second
  request and separate authentication/unavailability recovery sessions.
- Checkpoint 5 also verified durable inference start/terminal records, evidence
  retrieval after an OID process restart, and native metrics propagation: 4
  prompt tokens, 128 generated tokens, 132 context tokens, 3.22 tokens/sec,
  and 39,719 ms latency in the observed run.
- Cancelled, unavailable, and unauthorized requests produced terminal records;
  no raw prompt, response, bearer token, or provider secret was persisted.
- Checkpoint 6 verified independent Marina generation IDs, scoped model
  discovery/inference authorization, model-admin denial, cancellation followed
  by immediate same-daemon recovery, deterministic timeout followed by
  recovery, invalid authentication, and unavailable-port handling on the same
  macOS x86_64 daemon.
- Checkpoint 6 memory admission rejects unknown available memory and estimates
  model allocation plus context/KV-cache reserve and a conservative safety
  margin. It is not evidence of accelerator-specific memory support.
- The native llama.cpp path uses the model vocabulary detokenizer and has
  produced correctly spaced output in the verified completion.
- Context size, timeout, and conservative hardware-memory admission are
  enforced for the current native slice; multi-device and accelerator-aware
  accounting remain pending.
- The native CPU graph failure was reproduced in daemon stderr during
  Checkpoint 6 (`ggml_backend_sched_graph_compute_async` error 1 and
  `llama_decode ret=2`). Recovery and subsequent requests succeeded, but the
  root cause remains unestablished.
- Platform support must be verified with the release installer and native
  library payload for that target. Successful compilation on another platform
  does not update this matrix.
