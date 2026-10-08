# Marina Runtime Compatibility Matrix

This matrix records verified combinations. An empty or unverified cell is not
evidence of support.

| OS | Architecture | Backend | Format | Quantization | Transport | Status |
| --- | --- | --- | --- | --- | --- | --- |
| macOS | x86_64 | llama.cpp | GGUF | Q4_K_M | Unix socket + loopback HTTP | Local-only qualified; known cancellation diagnostic |
| macOS | arm64 | llama.cpp | GGUF | Q4_K_M | Unix socket + loopback HTTP | Blocked: no Apple Silicon host available |
| Linux | x86_64 | llama.cpp | GGUF | Q4_K_M | Unix socket + loopback HTTP | Qualified on Ubuntu 26.04 / AMD Phenom II X6; GitHub Actions baseline pending |
| Windows | x86_64 | llama.cpp | GGUF | Q4_K_M | Loopback TCP + HTTP | Blocked: no Windows host or runner available |

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
  `llama_decode ret=2`). Checkpoint 7B confirmed this is the pinned
  llama.cpp abort status when cancellation is active; the upstream logger
  remains error-looking but Marina classifies the terminal outcome correctly.
- Checkpoint 7 repeated three success, three cancellation, and three
  timeout/recovery sequences on macOS x86_64. It also verified self-cancel,
  cross-principal cancellation denial, unique generation IDs, and recovery
  after the native abort diagnostic. The pinned llama.cpp API defines
  `llama_decode` return value 2 as an abort and its context callback as an
  intentional decode abort; Marina now distinguishes that status from other
  native failures and from cancellation races. The upstream error-level graph
  logging remains a known diagnostic behavior.
- Platform support must be verified with the release installer and native
  library payload for that target. Successful compilation on another platform
  does not update this matrix.
- Checkpoint 8 platform inventory found only macOS x86_64 (`Darwin 21.6.0`) in
  the current workspace. Docker client support exists, but the Docker daemon
  is unavailable; no Linux or Windows native execution was performed, and no
  Apple Silicon host was available. These are blocked prerequisites, not
  qualification failures or passes.
- Qualification reports use the machine-readable schema
  `marina.runtime.qualification/v1` when `QUALIFY_REPORT` is supplied to
  `scripts/qualify-marina-http.sh`.
- WP-0078's current report is
  `docs/qualification/linux-x86_64-wp-0078.json`; that inventory report is
  superseded for Linux runtime status by the executed Phenom report below.
- WP-0078 has now executed on the Phenom II native host. Evidence is in
  `docs/qualification/linux-x86_64-wp-0078-phenom.json` and the governed OID
  evidence is summarized in
  `docs/qualification/linux-x86_64-phenom-oid-evidence.json`.
