# ADR-0007: Hardware-Aware Runtime

- Status: Accepted

## Context

Model feasibility and performance depend on available CPU, GPU, NPU, device memory, system memory, topology, and live health. A backend-neutral runtime that ignores hardware cannot make reliable admission or model-selection decisions.

## Decision

The runtime detects and normalizes available CPU, GPU, NPU, and memory resources. It uses those facts, model manifests, quantization variants, policy, and current load to select a compatible model and placement automatically. Hardware-specific behavior remains behind the hardware abstraction and backend adapters.

The initial Hardware Service reports operating system, architecture, CPU,
logical/physical cores, RAM, and available SIMD capabilities with explicit
GPU/NPU placeholders. Discovery is a platform service, not a vendor
optimization layer.

## Consequences

The runtime can make useful choices across heterogeneous machines and reject impossible requests before loading. Discovery is platform-specific, information can be incomplete, and automatic selection must expose its rationale and allow policy-bounded overrides.

## Alternatives Considered

- Requiring users to select devices and model variants shifts complex systems knowledge to every client.
- Leaving placement entirely to backends prevents consistent quotas, observability, and cross-backend policy.
- Supporting only CPU execution would simplify the first version but fail the project’s local-performance and accelerator goals.

## Future Considerations

Research NUMA, unified memory, device loss, multi-device placement, NPU APIs, thermal/power constraints, and quality-aware quantization selection.
