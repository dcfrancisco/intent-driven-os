# WP-0025: Hardware Discovery

## Purpose

Expose normalized platform facts needed for future model placement without coupling the runtime to vendor-specific optimizations.

## Scope

Operating system, architecture, CPU description, logical and physical cores, installed and available RAM, SIMD capabilities, and explicit GPU/NPU placeholders.

## Deliverables

- `HardwareService` and `HardwareSnapshot` APIs.
- Platform abstraction with portable fallbacks.
- Linux `/proc` discovery where available.
- Hardware-detected event.
- `hardware` console command output.

## Acceptance Criteria

- Discovery works without requiring an accelerator.
- Missing fields degrade to explicit unknown or placeholder values.
- No vendor-specific optimization or inference placement is implemented.
- Hardware data reaches the console through `RuntimeService`.

## Dependencies

WP-0005, WP-0018, WP-0020; ADR-0002, ADR-0007.

## Future Work

Add GPU/NPU providers, NUMA topology, device memory, health polling, and scheduler inputs.

