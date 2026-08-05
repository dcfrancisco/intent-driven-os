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

WPs may be refined or split as decisions mature. Dependencies indicate design order, not an implementation commitment.
