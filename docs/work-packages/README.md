# Work Packages

Work Packages (WPs) turn architectural decisions into reviewable bodies of design work. They are documentation and planning units; a WP does not authorize implementation outside its stated scope.

## Initial sequence

| ID | Work package | Primary outcome |
| --- | --- | --- |
| WP-0001 | Repository Foundation | Stable project structure and contribution baseline |
| WP-0002 | Runtime Architecture Documentation | Coherent runtime control-plane design |
| WP-0003 | Backend Adapter Interface | Backend-neutral adapter contract, without implementation |
| WP-0004 | Model Registry Design | Trusted model identity and manifest design |
| WP-0005 | Hardware Discovery Design | Normalized resource and capability model |
| WP-0006 | AI Console Design | Retro, keyboard-first console UX |
| WP-0007 | Command Grammar | Intent-based command language design |
| WP-0008 | Model Lifecycle | Model state and operational workflows |
| WP-0009 | Observability | Logging, metrics, tracing, and audit design |
| WP-0010 | Security Architecture | Identity, policy, permissions, and approvals |
| WP-0017 | Adaptive Cursor System | Runtime-aware cursor UX and theme contract |
| WP-0018 | Runtime Event Bus | Lightweight runtime-to-console event delivery |
| WP-0019 | Status Bar | Persistent runtime health and resource summary |
| WP-0020 | Mock Runtime | Deterministic runtime service for client validation |
| WP-0021 | Startup Experience | Fast, legible console initialization flow |
| WP-0022 | Backend Interface | Common lifecycle and generation contract |
| WP-0023 | Backend Registry | Runtime-owned backend registration and selection |
| WP-0024 | Model Registry | Persistent model metadata management |
| WP-0025 | Hardware Discovery | Platform-neutral hardware service |
| WP-0026 | llama.cpp Adapter Skeleton | Contract-only first backend adapter |
| WP-0027 | Native llama.cpp Integration | Direct official C API lifecycle and health integration |
| WP-0028 | Model Discovery | Configurable recursive GGUF discovery |
| WP-0029 | Model Loader | Single-model load/unload lifecycle and memory accounting |
| WP-0030 | Tokenizer | Backend-routed UTF-8 token counting |
| WP-0031 | Inference Engine | Backend-neutral generation contracts |
| WP-0032 | Streaming Generation | Non-blocking token streams and cancellation |
| WP-0033 | Console Integration | `generate`, `complete`, and `explain` commands |
| WP-0034 | Runtime Metrics | Prompt, output, latency, throughput, and context metrics |
| WP-0035 | Cancellation | Request cancellation without process termination |
| WP-0036 | Generation Events | Observable generation lifecycle events |
| WP-0037 | Dynamic CLI Capability Framework | Native, intent, and runtime-registered capability command routing |
| WP-0038 | Operation Planning Framework | Canonical plan, approval, execution, verification, and rollback lifecycle |
| WP-0039 | Operation Registry and Allowlist | Governed registration and native-command collision protection |
| WP-0040 | Native/Dynamic/Intent Input Router | Deterministic three-path terminal input classification |
| WP-0041 | Dynamic Capability Registration | Runtime capability lifecycle and discovery |
| WP-0042 | Help and Completion Integration | Discoverable help and completion for loaded capabilities |
| WP-0043 | Durable Approval and Recovery | Append-only approval journal and recoverable execution state |
| WP-0044 | Read-only Skill Expansion | Directory and file inspection through governed plans |
| WP-0045 | Operation Coordinator | End-to-end governed lifecycle orchestration |
| WP-0046 | Persistent Operation State Machine | Append-only operation state and recovery inspection |
| WP-0047 | Native Command Execution Adapter | Explicitly approved native executable execution |
| WP-0048 | Dynamic Capability Execution Adapter | Registered capability dispatch through skills |
| WP-0049 | Evidence and Verification Correlation | Operation-linked plan, execution, and verification records |
| WP-0050 | Recovery and Resume CLI Boundary | Recoverable operation API for terminal integration |
| WP-0051 | Process Inspection Skill | Governed Linux process-table inspection |
| WP-0052 | Filesystem Inspection Skill | Governed filesystem capacity inspection |
| WP-0053 | Systemd Health Adapter | Read-only systemd manager health boundary |
| WP-0054 | D-Bus Desktop Boundary | Isolated read-only D-Bus transport contract |
| WP-0055 | Terminal Identity and Shell Passthrough | Real Linux shell with explicit `:`-prefixed OID commands |
| WP-0056 | Process and Signal Reliability | Reliable interruption, shutdown, child cleanup, and recovery |
| WP-0057 | Local Runner Contract Hardening | Versioned runner semantics, manifests, request identity, and failure taxonomy |
| WP-0058 | Verified Model Cache and Admission | Trusted model artifacts with memory/context/resource admission |
| WP-0059 | Runtime API and Stream Transport | Versioned Unix-socket model/run/health API with idempotency |
| WP-0060 | Backend Capability and Router | Deterministic, explainable local-first backend selection |
| WP-0061 | Compatibility Edges | Scoped Ollama and OpenAI-compatible translation with capability gaps |
| WP-0062 | Runtime Recovery and Observability | Crash isolation, metrics, audit records, and deterministic recovery |
| WP-0063 | Assistant Session and Tool Boundary | Governed sessions/tools after runner and router foundations are stable |
| WP-0064 | Individual Model Runner Installer | Installable Marina service/client without the interactive console |
| WP-0065 | Cross-Platform Runner Packaging | Linux, macOS, and Windows user/system installer artifacts |
| WP-0066 | Standalone Service Transports | Portable local service lifecycle, health, and IPC/API transports |
| WP-0067 | Assistant API Compatibility Edge | Versioned assistant API and OpenAI-compatible streaming surface |
| WP-0068 | OID Runtime Integration | OID and system clients consume Marina through the public API |
| WP-0069 | Linux Distro Packaging and Service | FHS-aligned packages, systemd units, upgrades, rollback, and removal |
| WP-0070 | AI Capability and Provider Contract | Versioned messages, capabilities, sessions, tools, embeddings, and provider model |
| WP-0071 | Runtime Security and Resource Isolation | Service sandboxing, secrets, quotas, device access, and data policy |
| WP-0072 | AI Desktop Self-Management | Context, skills, approvals, maintenance, verification, and recovery |
| WP-0073 | OID OS-Management Model | Training, evaluation, provenance, and Marina integration |
| WP-0074 | Rust CA-Clipper Linux Subsystem | Legacy inventory, Rust rewrite, data compatibility, and Linux packaging |
| WP-0075 | Product Brand and Launch Language | Product names, positioning, release boundaries, and claim discipline |
| WP-0076 | Marina Training and Model Lifecycle | Governed datasets, training jobs, evaluation, promotion, and rollback |
| WP-0077 | Marina Native Transport and API | Native HTTP, persistent listeners, auth, lifecycle, and admission |
| WP-0078 | Marina Linux Native Qualification | Native Linux x86_64 runtime and release qualification |
| WP-0079 | Marina Windows Native Qualification | Native Windows x86_64 runtime and release qualification |
| WP-0080 | Marina macOS arm64 Qualification | Native Apple Silicon runtime qualification |
| WP-0081 | Marina Remote Transport Qualification | Authenticated private/TLS remote deployment qualification |
| WP-0082 | Marina Training Contracts | Backend-neutral Phase 1 training schemas and boundaries |
| WP-0083 | Marina Dataset Registration | Immutable dataset identity, validation, provenance, and hashes |
| WP-0084 | Marina Training Job Lifecycle | Isolated jobs, scheduling, cancellation, checkpoints, and limits |
| WP-0085 | Marina CPU Training Backend | Bounded real CPU-compatible small-model training |
| WP-0086 | Marina Training Evaluation | Baselines, evaluation, promotion gates, activation, and rollback |
| WP-0087 | Marina Artifact Lineage | Immutable model, adapter, checkpoint, and conversion lineage |

WPs may be refined or split as decisions mature. Dependencies indicate design order, not an implementation commitment.
