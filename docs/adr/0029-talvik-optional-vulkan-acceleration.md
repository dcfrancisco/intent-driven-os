# ADR-0029: Optional Vulkan Acceleration for Talvik

- Status: Proposed
- Date: 2026-10-27
- System: Talvik Model Runtime
- Related: ADR-0003, ADR-0007, ADR-0010, ADR-0013, ADR-0024;
  WP-0027, WP-0058, WP-0060, WP-0078, WP-0079, WP-0080, WP-0090, WP-0105,
  WP-0106

## Context

Talvik currently integrates llama.cpp behind a backend-neutral Rust runtime
boundary and is CPU-first. Its native build configuration does not establish
that a Vulkan backend is compiled, that a GPU is available to llama.cpp, or
that any model layers are actually placed on a GPU. OID's inference request
contract must remain unchanged when execution is accelerated.

The Windows host probe on 2026-10-27 ran `vulkaninfo --summary` and enumerated
an integrated Intel(R) Iris(R) Xe Graphics device. The instance reported
Vulkan 1.4.313, the device reported Vulkan 1.4.323, and the Vulkan driver
version was 101.7080. This verifies that the Vulkan loader can enumerate the
device on this host. It does not verify llama.cpp Vulkan support, Talvik device
selection, model-layer offloading, usable GPU memory, or inference
performance. The probe did not report an adapter-memory size.

## Decision

Talvik should add optional Vulkan acceleration through its existing llama.cpp
adapter, without introducing an Intel-specific runtime path, changing the
inference API, requiring CUDA, or weakening CPU-only support.

The implementation work package SHALL:

- Keep CPU execution available and buildable when Vulkan is not installed,
  compiled, or selected. Vulkan support is an optional build capability, not a
  required workspace dependency.
- Keep Vulkan, ggml, llama.cpp, native device handles, and vendor-specific
  memory APIs inside the native adapter boundary. The runtime exposes only
  backend-neutral device and placement facts.
- Support explicit `auto`, `cpu`, and `vulkan` selection, configurable GPU
  layer offloading, and an explicit CPU-fallback policy. The final config
  serialization and defaults must follow Talvik's existing configuration
  conventions.
- Enumerate the backends and devices that are actually compiled and usable.
  A host-level Vulkan device enumeration is not evidence that Talvik can use
  Vulkan.
- Distinguish requested offload settings from verified actual placement.
  Report actual offloaded layers only when the pinned llama.cpp/ggml version
  provides a reliable observable; otherwise report that placement is unknown.
- Allow `auto` to select a GPU only when device compatibility, resource
  admission, and a qualification profile establish that route as eligible.
  Prefer the fastest qualified choice for the model and host; do not assume a
  GPU is faster. Until a GPU route is qualified, CPU remains the safe fallback.
- On configured fallback, clean up a failed GPU load before retrying on CPU
  and report the attempted backend, selected backend, and fallback reason.
  Explicit Vulkan selection with fallback disabled must fail visibly instead
  of silently switching backends.
- Account for system RAM and device memory separately where the platform
  reports distinct pools. For integrated/shared-memory GPUs, avoid
  double-counting the same physical memory as both dedicated VRAM and system
  RAM; unknown capacity must remain unknown or use conservative admission.
- Preserve request cancellation, deadlines, model residency, single-generation
  admission, eviction, and recovery behavior.
- Keep the public inference request/response semantics stable. Additive
  diagnostics may report compiled backends, devices, selected execution
  backend, memory observations, offload facts, and fallback reasons.

## Alternatives considered

- Making Vulkan mandatory would break CPU-only builds and hosts without a
  Vulkan SDK, driver, or compatible device.
- Hardcoding Intel Iris Xe support would fail the backend-neutral design and
  exclude other Vulkan-capable GPUs.
- Treating `vulkaninfo` or a requested GPU-layer count as proof of offloading
  would produce misleading diagnostics.
- Automatically preferring any detected GPU would ignore compatibility,
  shared-memory pressure, and cases where CPU inference is faster.
- Replacing llama.cpp or adding a second scheduler would expand scope without
  improving the existing adapter boundary.

## Consequences

Talvik can evolve from CPU-first operation toward qualified heterogeneous
execution while preserving backend neutrality and CPU compatibility. Native
builds, FFI bindings, resource admission, diagnostics, and release evidence
must agree on what was actually built, selected, allocated, and used.

The initial Intel host is a discovery target, not a performance or product
qualification. A Vulkan build and an actual GPU inference run on supported
hardware are required before Talvik may claim Iris Xe acceleration. CPU and
Vulkan measurements must use the same model artifact, llama.cpp revision,
prompt/context, token limits, sampling options, and test procedure.

## Implementation work package

WP-0106 SHALL deliver the optional Vulkan llama.cpp path, backend-neutral
runtime reporting, safe fallback/admission behavior, and a repeatable CPU
versus Vulkan qualification procedure. GPU qualification remains pending
until real hardware evidence is recorded.
