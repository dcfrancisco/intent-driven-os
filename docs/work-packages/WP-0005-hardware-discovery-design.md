# WP-0005: Hardware Discovery Design

## Purpose

Define a portable hardware abstraction that gives the runtime enough information to make safe model and placement decisions.

## Scope

CPU, GPU, NPU, system/device memory, topology, capabilities, driver/runtime information, live health, and unknown or degraded hardware states.

## Deliverables

- Normalized hardware resource model.
- Discovery ownership and platform adapter boundaries.
- Capability and memory reporting requirements.
- Health, device-loss, and quarantine states.
- Inputs to quantization selection, memory admission, and scheduling.

## Acceptance Criteria

- CPU-only, accelerator-present, and partially discoverable systems are represented.
- Hardware facts are separated from policy decisions.
- Stale, missing, and unsafe device data have defined behavior.
- The model supports future GPU/NPU vendors without changing client contracts.

## Dependencies

WP-0002; ADR-0002, ADR-0007.

## Future Work

Add NUMA, unified-memory, power/thermal, multi-device, and vendor-specific performance profiles.

