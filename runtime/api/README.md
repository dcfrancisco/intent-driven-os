# Runtime API Contract

This directory describes the future public contract between runtime clients and the Intelligent Runtime. It contains no implementation or generated protocol code.

## API domains

- **Models:** list, pull, inspect, verify, cache status, load, unload.
- **Runs:** start, stream, stop, cancel, status, and result metadata.
- **Hardware:** CPU/GPU/NPU inventory, capabilities, health, and current allocations.
- **Backends:** list, capabilities, health, enable, disable, and version information.
- **Policy:** caller identity, authorization decisions, quotas, limits, and admission reasons.
- **Operations:** health, metrics, audit events, and tracing correlation.

## Contract rules

1. Every request has a caller identity, correlation ID, API version, and deadline where applicable.
2. Every model reference resolves to a manifest and immutable artifact identity before loading.
3. Streaming responses are ordered, cancellable, backpressure-aware, and terminate with an explicit completion or error event.
4. Admission failures are structured and distinguish policy, capacity, capability, integrity, and backend errors.
5. Backend-specific fields are optional extensions and cannot be required by the common interface.
6. Long-running operations expose state and can be queried after client disconnect.
7. Authorization applies to model data, hardware, backend administration, and audit data independently.

## Transport direction

Unix socket is the initial transport. The socket should use filesystem permissions and peer credentials for local authentication. Optional TCP requires explicit configuration, TLS, authentication, authorization, rate limiting, and audit coverage; it must not be enabled implicitly.

