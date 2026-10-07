# WP-0071: Runtime Security and Resource Isolation

## Purpose

Make a standalone Marina service safe to operate on a developer workstation,
desktop, or shared Linux host.

## Scope

- Caller identity and local socket/HTTP authentication.
- Secret-store references and redaction rules.
- Model trust, download verification, source policy, and offline mode.
- Memory/context quotas, concurrency, rate limits, and cancellation limits.
- systemd sandbox profiles and backend-specific GPU/device permissions.
- Network egress policy, remote-provider consent, data residency, and audit.
- Crash isolation, restart limits, circuit breakers, and recovery evidence.

## Acceptance criteria

- Local and remote fallback policy is explicit and machine-auditable.
- No model/tool/provider credential crosses an unauthorized boundary.
- Resource exhaustion and backend crashes do not take down the host or corrupt
  model state.
- Security tests cover user service, system service, HTTP exposure, offline
  mode, model import, and tool-call proposal flows.

## Dependencies

WP-0058, WP-0062, WP-0066, WP-0069, WP-0070; ADR-0006, ADR-0014, ADR-0015.
