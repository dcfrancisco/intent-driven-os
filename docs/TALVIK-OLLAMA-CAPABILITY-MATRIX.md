# Talvik/Ollama Capability Matrix

Date: 2026-10-23
Talvik branch: `feature/talvik-m2-runtime-ide`
Ollama reference: `/Users/dannyfrancisco/codes/ollama` at
`f52b9e742d8c9a308f7b23cd1f0ab52c7ddb694d`

This is the authoritative disposition of relevant Ollama-style capabilities.
“Verified” requires executable evidence on the current source; code without
native, external-client, or platform evidence remains explicitly qualified as
incomplete.

| ID | Capability | Talvik evidence/status | Ollama evidence | Gap or impact | Priority | ADR | WP | Dependency | Acceptance test | Environment | Disposition |
|---|---|---|---|---|---|---|---|---|---|---|---|
| CAP-001 | Native GGUF/llama.cpp loading | `runtime/adapters/llama-cpp`, native loading; implemented | `server/sched.go` runner lifecycle | Native host evidence incomplete outside recorded macOS x86_64 | P0 | ADR-0025 | WP-0091 | Native library/GGUF | Load, generate, unload | Native hosts | Implemented but not qualified |
| CAP-002 | Multiple resident models | `MarinaRuntime.loaded`, resident snapshots, LRU recovery; implemented | Loaded runner map and keyed reuse in `server/sched.go` | Simultaneous native residency per platform incomplete | P0 | ADR-0025 | WP-0091 | CAP-001 | A→B→A, unload B | Each native host | Implemented but not qualified |
| CAP-003 | FIFO queue and one active generation | `pending_generations`, `active_generation`; implemented | `pendingReqCh` and scheduler dispatch | Native timing and queue evidence incomplete | P0 | ADR-0025 | WP-0091 | CAP-002 | FIFO, overflow, cancellation | Native + fixtures | Implemented but not qualified |
| CAP-004 | Admission and bounded LRU eviction | `admit_memory`, `eviction_candidate`; implemented | Memory placement/eviction in `server/sched.go` | Estimates where backend measurements unavailable | P0 | ADR-0025 | WP-0093 | CAP-002 | Pressure, pinning, recovery | Native + fixtures | Implemented but not qualified |
| CAP-005 | Build/runtime identity | `console/build.rs`, `build_info.rs`, CLI, IPC, HTTP; verified | Ollama version surfaces | Native revision is null when unavailable | P0 | ADR-0025 | WP-0095 | Cargo build | CLI/version/status/API | macOS x86_64 | Verified |
| CAP-006 | Client/daemon mismatch reporting | IPC status carries both versions and comparison | Client/server API versioning | Live mismatched daemon test pending | P0 | ADR-0025 | WP-0095 | CAP-005 | Mismatched client/daemon | Native daemon | Implemented but not qualified |
| CAP-007 | OpenAI model discovery | Authenticated `/v1/models` | Ollama tags/OpenAI edge | External client evidence incomplete | P0 | ADR-0025 | WP-0092 | Authentication | Valid/invalid auth | Local HTTP + IDE | Implemented but not qualified |
| CAP-008 | OpenAI chat/completions/SSE | `/v1/chat/completions`, `/v1/completions`; implemented | Ollama generation routes | IDE/client evidence incomplete | P0 | ADR-0025 | WP-0092 | CAP-007 | Streaming, finish, cancellation | Python/OpenAI + IDE | Implemented but not qualified |
| CAP-009 | OpenAI embeddings | `/v1/embeddings` and native backend; implemented | Ollama embed capability | Native platform evidence incomplete | P0 | ADR-0025 | WP-0092 | CAP-001 | Shape, auth, real GGUF | Native host | Implemented but not qualified |
| CAP-010 | Tool calls/tool results | Template-aware support and explicit rejection; partial | Model/template dependent Ollama behavior | Real IDE tool workflow unavailable | P0 | ADR-0025 | WP-0092 | CAP-008 | Tool call/result round trip | Cline/compatible client | Implemented but not qualified |
| CAP-011 | Ollama `/api/tags` | Authenticated route; implemented | `server/routes.go` | Artifact size intentionally unknown | P0 | ADR-0025 | WP-0091 | Auth | Auth success/failure | Local HTTP | Implemented but not qualified |
| CAP-012 | Ollama `/api/ps` | Actual resident snapshots; implemented | `server/routes.go` | Per-request keep-alive differs | P0 | ADR-0025 | WP-0091/WP-0093 | CAP-002 | Resident/unload visibility | Native host | Implemented but not qualified |
| CAP-013 | Ollama `/api/show` | Missing; model inspection exists elsewhere | Ollama model inspection route | No selected M2 client requires it | P1 | ADR-0025 | WP-0096 | Client demand | Real client need | N/A | Deferred to M2.1 |
| CAP-014 | Ollama `/api/chat`, `/api/generate` | Missing | Ollama generation routes | Duplicate stable OpenAI edge; no selected client need | P1 | ADR-0025 | WP-0096 | Client demand | Compatibility test with client | N/A | Deferred to M2.1 |
| CAP-015 | Ollama `/api/embed` | Missing | Ollama embedding route | `/v1/embeddings` is supported edge | P1 | ADR-0025 | WP-0096 | Client demand | Real client need | N/A | Deferred to M2.1 |
| CAP-016 | Keep-alive/idle expiration | Configurable retention/eviction; partial | `keep_alive` and expiration timers | Per-request keep-alive not exposed | P0 | ADR-0025 | WP-0093 | CAP-004 | Expiry and pinned protection | Native host | Implemented but not qualified |
| CAP-017 | Pin/unpin | Internal policy protection; public control incomplete | Runner reference protection | CLI/API pin semantics incomplete | P0 | ADR-0025 | WP-0093 | CAP-004 | Pin, pressure, unpin | Native + fixtures | Partial |
| CAP-018 | Model metadata/manifests | Registry has checksum, family, quantization, template/capabilities; partial | Ollama manifests/digests/templates | No full OCI distribution by design | P1 | ADR-0025 | WP-0097 | Registry | Import/inspect/integrity | Local | Partial |
| CAP-019 | Aliases/deduplication | IDs/checksums exist; alias policy incomplete | Ollama name/digest identity | Stable alias and dedupe evidence needed | P1 | ADR-0025 | WP-0097 | CAP-018 | Same artifact aliases | Local | Deferred to M2.1 |
| CAP-020 | Concurrent inference | Serialized one-active baseline intentionally retained | Scheduler concurrency constraints | No native safety/throughput evidence yet | P1 | ADR-0025 | WP-0094 | Native measurements | Compare parallel/serialized | Native hosts | Deferred pending measurements |
| CAP-021 | Operational diagnostics | Snapshots, status, `/api/ps`, structured errors; partial | Scheduler and `/api/ps` diagnostics | Counters/wait/eviction records need expansion | P1 | ADR-0025 | WP-0091 | Runtime events | Inspect without prompt content | Local/native | Implemented but not qualified |
| CAP-022 | Continue IDE interoperability | Procedure prepared; no real session | Client-specific | IDE unavailable here | P0 | ADR-0025 | WP-0092 | IDE install/model | Stream coding task, prove provider | Owner host | Blocked |
| CAP-023 | Cline tool workflow | Procedure prepared; no real session | Client-specific | IDE unavailable here | P0 | ADR-0025 | WP-0092 | CAP-010 | Tool/result/cancel | Owner host | Blocked |
| CAP-024 | Codex custom provider | Not verified | Not an Ollama server property | Must follow installed client support | P1 | ADR-0025 | WP-0092 | Client version | Documented provider path | Owner host | Deferred by client capability |
| CAP-025 | GPU-aware scheduling | Not implemented by CPU-first scope | Ollama placement logic | Separate resource design required | P2 | ADR-0025 | WP-0100 | GPU hosts | VRAM admission/recovery | Future | Scheduled M3 |
| CAP-026 | GPU containers | Not implemented | Platform-specific deployment | Docker CPU is current scope | P2 | ADR-0025 | WP-0100 | CAP-025 | GPU container smoke | Future | Scheduled M3 |
| CAP-027 | Multimodal support | Not implemented | Model-dependent Ollama APIs | No requirement/model evidence | P2 | ADR-0025 | WP-0101 | Capability contract | Real multimodal client | Future | Scheduled M3 |
| CAP-028 | MarinaOS service integration | Explicitly out of scope | Not an Ollama capability | Future product integration | P2 | ADR-0025 | WP-0102 | MarinaOS architecture | Separate adapter design | Future | Rejected for M2; scheduled later |
| CAP-029 | Additional inference engines | Explicitly out of scope | Not a Talvik dependency | llama.cpp-only constraint | P2 | ADR-0025 | WP-0103 | Separate authorization | New ADR/backend contract | Future | Rejected for M2 |

No capability may be removed from this table. Promotion to Verified requires
the listed acceptance evidence. Deferred, Blocked, and Scheduled rows remain
owned by their listed work package.
