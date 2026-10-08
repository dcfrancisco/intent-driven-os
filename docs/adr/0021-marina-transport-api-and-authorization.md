# ADR-0021: Marina Transport, API, and Authorization Architecture

- Status: Proposed
- Date: 2026-10-08
- System: Marina Model Runner
- Related systems: Intent-Driven OS (OID), IDEs, Athena Hub, Delivery Wizard
- Category: Transport / API / authorization
- Priority: P1 prerequisite for ADR-0020
- Related work: WP-0077; ADR-0012, ADR-0013, ADR-0015, ADR-0020

## Context

Marina currently has a useful local daemon boundary and a Python
OpenAI-compatible bridge. The bridge is a development and compatibility edge,
not the permanent service architecture. Training, OID integration, IDE access,
and other clients must use a stable Marina-owned API rather than implementing
separate networking, authentication, cancellation, or resource controls.

Transport, protocol, and authorization are separate concerns:

- **Transport** establishes connectivity: Unix socket, TCP, named pipe, or
  HTTP listener.
- **Protocol** defines request/response semantics: Marina's versioned API and
  compatibility edges such as OpenAI chat/completions.
- **Authorization** determines which connected principal may perform which
  operation, subject to policy, scopes, quotas, and resource admission.

## Decision

Marina SHALL provide a first-class native HTTP server before implementing the
ADR-0020 training lifecycle. The first native HTTP slice is now implemented;
the Python bridge remains a migration,
compatibility, and test edge until equivalent native HTTP behavior is stable.

Marina SHALL preserve backward compatibility:

- Linux/macOS retain the user-owned Unix socket by default.
- Windows retains configurable loopback TCP by default.
- Existing IDE integrations continue to work during migration.
- TCP/HTTP binding remains loopback-only unless explicitly configured with
  authenticated private connectivity and deployment policy. The current
  implementation rejects non-loopback addresses unless
  `MARINA_HTTP_ALLOW_NON_LOOPBACK=true` is explicitly set; that override is
  for secured deployments only and is not a substitute for network policy.

The native server SHALL expose the versioned Marina API and may expose
OpenAI-compatible routes as an edge translation. OpenAI protocol types and
provider-native types must not become Marina's internal model.

## Proposed configuration

The initial implementation preserves the current single-transport defaults;
the schema reserves a listener list for future multiple listeners:

```yaml
server:
  listeners:
    - name: local
      transport: unix
      path: ~/.marina/marina.sock
    # Explicit opt-in only:
    # - name: http
    #   transport: tcp
    #   bind_address: 127.0.0.1
    #   port: 11434
  authentication:
    enabled: true
  limits:
    max_concurrent_requests: 4
    max_queued_requests: 32
  cancellation:
    enabled: true
```

The current dependency-free YAML loader may initially accept flattened keys;
the public schema must converge on the nested form before declaring the
configuration contract stable. Environment variables remain overrides for
automation and compatibility.

Configuration precedence is environment variable, then nested YAML, then
legacy flat YAML alias. Authentication is enabled by default. The current
implementation accepts the first configured listener and does not yet claim
multi-listener scheduling.

## Authorization and client connectivity

Unix-socket access relies on user-owned socket and parent-directory
permissions, with credential-based authentication available for clients that
need explicit identity. HTTP access requires authentication and scoped
authorization. OID, interactive users, IDEs, and other applications are
separate local principals even when they share a machine.

The initial scope vocabulary is:

- `models:read` for model discovery;
- `inference` for generation;
- `inference:cancel:self` for cancelling a generation admitted by the same
  principal;
- `inference:cancel:any` for explicitly authorized cross-principal
  cancellation; and
- `models:admin` for model load/unload/acquisition administration.

The HTTP control plane owns the request-to-principal ownership map. Marina
assigns a separate opaque generation ID (`marina-gen-*`) and preserves the
client request ID as a correlation field. Cross-client cancellation is denied
unless the caller has the administrative cancellation scope.

Remote OID integration must initially use a secured tunnel or authenticated
private connection. Marina must not be exposed directly to a LAN or public
network by default.

## Request lifecycle and admission

The native API owns request identity, streaming terminal events, cancellation,
timeouts, concurrency limits, queue limits, memory/context admission, and
metrics. Admission is required before serious OID integration so concurrent
requests cannot exhaust model memory. Training and inference share these
platform capabilities, but production inference has configurable priority over
background training.

The current admission policy rejects when available system memory is unknown
and otherwise reserves the loaded model allocation, a conservative context/KV
estimate, and a fixed safety margin. It remains a first local-runtime policy,
not a complete accelerator allocator.

## Implementation order

1. Native HTTP server and versioned API resources.
2. Persistent transport/listener configuration.
3. Authentication, authorization, and scopes.
4. Request lifecycle: cancellation, timeouts, quotas, and limits.
5. OID client integration over the public API.
6. Resource and context admission.
7. Multi-backend routing.
8. ADR-0020 training, evaluation, promotion, and rollback.

## Consequences

Marina becomes independently usable by OID, IDEs, Athena Hub, Delivery Wizard,
and other applications. The Python bridge can be retired without changing the
internal runtime. The project accepts additional implementation work for a
native HTTP stack, persistent configuration, scoped authorization, and API
conformance before training begins.

## Non-goals

- Exposing Marina directly to the LAN by default.
- Making OpenAI compatibility the internal architecture.
- Allowing transport connectivity to bypass authorization or admission.
- Implementing training endpoints before this transport and lifecycle
  foundation is stable.
