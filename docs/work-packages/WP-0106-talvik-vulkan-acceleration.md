# WP-0106: Talvik Optional Vulkan Acceleration

## Status

In progress. Runtime/HTTP placement reporting and `oid-marina-chat` timing and
benchmark display are implemented for the current CPU path. The current
Windows host exposes an Intel Iris Xe Vulkan device, but Talvik Vulkan
compilation, model offloading, inference, and performance have not been
qualified.

## Purpose

Add optional llama.cpp Vulkan acceleration to Talvik while preserving its
backend-neutral inference contract and CPU-only operation across Windows,
Linux, and macOS.

## Scope

- Inspect the pinned llama.cpp/ggml revision, native build scripts, FFI,
  adapter loading path, hardware discovery, model admission, and diagnostics
  before selecting implementation APIs.
- Add an opt-in Vulkan build path for supported platforms; keep CPU-only builds
  working without Vulkan headers, SDKs, drivers, or libraries.
- Use the pinned llama.cpp/ggml device and model-parameter APIs to pass actual
  device selection and GPU-layer settings through the native loader.
- Add backend-neutral runtime configuration for `auto`, `cpu`, and `vulkan`,
  GPU-layer selection, and explicit CPU fallback. Reuse existing
  configuration and admission patterns rather than creating a second
  scheduler.
- Enumerate compiled/available backends and devices. Keep host Vulkan discovery
  distinct from runtime backend availability.
- Report the selected backend, requested offload, verified actual offloaded
  layer count when observable, and explicit fallback diagnostics.
- Extend resource observations and admission for device memory, model weights,
  KV cache, compute buffers, and safety margins. Account for shared/unified
  memory without double-counting physical RAM.
- Preserve API compatibility, model residency, cancellation, timeout recovery,
  scheduling/admission, eviction, and CPU fallback.
- Provide repeatable CPU-versus-Vulkan benchmarks for
  `Qwen2.5-0.5B-Instruct-Q4_K_M`, using a pinned model artifact and llama.cpp
  build. Record prompt-processing and generation throughput separately when
  the benchmark tooling supports them, plus memory, device, and observed layer
  placement.
- Document `talvik doctor` or the existing diagnostics surface for GPU model,
  driver, Vulkan availability, compiled/selected Talvik backend, GPU
  offloading state, actual layer placement, memory observations, and fallback
  reason. Do not add a separate diagnostics service.
- Report selected execution backend/device and verified actual layer placement
  in `oid-marina-chat --timing` and on each `oid-marina-chat --benchmark`
  sample. The benchmark summary must make mixed CPU/Vulkan runs visible, and
  unverified placement must remain explicitly unknown.

## Non-goals

- Rewriting the inference engine or introducing a second model scheduler.
- Requiring CUDA, NVIDIA hardware, Vulkan on CPU-only hosts, or a specific
  Intel adapter.
- Changing OID's inference contract or exposing native GPU handles through the
  runtime API.
- Claiming acceleration based only on device discovery or requested offload.
- Selecting a GPU for `auto` before it is compatible, admitted, and qualified.
- Marking Vulkan qualified or faster without successful real-device runs and
  recorded measurements.

## Acceptance criteria

- The normal CPU-only workspace and release builds remain usable on Windows,
  Linux, and macOS without Vulkan installed.
- Vulkan is an explicitly enabled native build feature and the supported
  Windows Vulkan build completes with the documented pinned dependencies.
- Runtime diagnostics distinguish: backend compiled, Vulkan device detected,
  device eligible, backend selected, actual offload verified, and fallback
  performed.
- `cpu` always provides a CPU route. `vulkan` either uses the requested
  supported route or fails visibly; configured fallback cleans up and
  retries on CPU while preserving the original failure reason.
- `auto` selects only eligible, qualified routes and records its selection
  rationale; an unqualified GPU falls back to CPU.
- Requested and actual offloaded layer counts are distinct. If the pinned
  native API cannot verify actual placement, diagnostics say unknown.
- `oid-marina-chat --timing` displays selected execution backend/device and
  actual placement; `--benchmark` displays them per run and identifies mixed
  CPU/Vulkan results in its summary.
- Resource admission includes available device memory and memory-pool
  semantics, including shared-memory devices, without double-counting.
- Repeated generation, GPU allocation failure, cancellation, timeout, model
  unload/reload, and CPU fallback tests show bounded resource use and preserve
  existing lifecycle behavior.
- Same-host CPU and Vulkan benchmarks record the model ID and checksum,
  hardware/driver, llama.cpp revision/build, context and generation options,
  warm/load state, prompt-processing and generation throughput when
  available, memory, actual offloaded layers, and run methodology.
- No performance claim is made until the Intel Iris Xe hardware qualification
  has been executed and its evidence reviewed. Linux Vulkan and macOS CPU-only
  regression results are recorded independently.

## Preliminary Windows host probe

`vulkaninfo --summary` was run on 2026-10-27. It reported:

| Property | Result |
| --- | --- |
| Vulkan instance version | 1.4.313 |
| Device | Intel(R) Iris(R) Xe Graphics |
| Device type | Integrated GPU |
| Device Vulkan API version | 1.4.323 |
| Vendor ID | `0x8086` |
| Vulkan driver | Intel Corporation; version `101.7080` |

The loader printed a Windows registry layer-manifest warning, but still
enumerated the Intel device. No adapter-memory capacity was reported in this
summary. This is host discovery evidence only; it does not demonstrate that
the current Talvik build includes Vulkan or offloads any model layers.

## Standalone Windows llama.cpp Vulkan build (2026-10-27)

The pinned llama.cpp revision `18443257a30c884d5332abb8e7dc43c7ffe42fda`
was built with `GGML_VULKAN=ON` using Vulkan SDK 1.4.363.0 and the installed
Visual Studio C++ toolchain. CMake reported `Found Vulkan` and `Including
Vulkan backend`. The resulting `llama-cli --list-devices` output reported:

```text
Vulkan0: Intel(R) Iris(R) Xe Graphics (8018 MiB, 7361 MiB free)
```

This confirms the pinned standalone llama.cpp build can enumerate the Iris Xe.
The reported memory is not evidence of dedicated VRAM on this integrated GPU.
No model was loaded, so this does not verify layer offloading, Talvik linkage,
inference correctness, memory admission, fallback, or performance.

## Qualification procedure

1. Record the pinned llama.cpp and Vulkan SDK/toolchain versions. Verify
   `vulkaninfo --summary` enumerates the target device.
2. Build the CPU-only Talvik variant and verify the existing CPU inference
   regression sequence.
3. Build the Vulkan-enabled llama.cpp/Talvik variant and record the enabled
   ggml backends and `llama-cli --list-devices` output.
4. Run the fixed Qwen2.5-0.5B-Instruct-Q4_K_M artifact through CPU-only and
   Vulkan paths with identical prompt, context, token limits, options, and
   warm/cold methodology. A `llama-bench` command may be used where supported
   by the pinned revision; record the exact command and output.
5. Verify actual model-layer placement and memory behavior through supported
   llama.cpp/ggml diagnostics. If placement cannot be verified, do not mark
   offloading as enabled or the GPU qualification as passed.
6. Exercise GPU load/allocation failure, explicit fallback, cancellation,
   timeout, repeated requests, and unload/reload. Verify CPU fallback is
   visible and leaves no unbounded native allocations.
7. Record prompt-processing throughput, token-generation throughput, model
   load time, TTFT, memory observations, actual layers offloaded, and any
   unavailable metrics. Compare only measurements collected under identical
   conditions.
8. Run Linux Vulkan regression where supported and macOS CPU-only regression.
   Keep each platform's evidence distinct.

## Dependencies

ADR-0029; ADR-0007, ADR-0010, ADR-0024; WP-0027, WP-0025, WP-0058, WP-0060,
WP-0078, WP-0079, WP-0080, WP-0090, WP-0105.

## Next dependency

Review the ADR proposal and confirm the pinned llama.cpp Vulkan/device APIs,
runtime configuration ownership, and admission data source before native
implementation begins.
