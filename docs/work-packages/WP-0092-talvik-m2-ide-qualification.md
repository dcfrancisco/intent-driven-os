# WP-0092: Talvik M2 IDE API Compatibility and Qualification

## Purpose

Qualify Talvik as a real local model provider for an external coding
assistant, starting with Continue and documenting Codex/Cline capability limits.

## Scope

- Provide executable Continue configuration against Talvik's OpenAI edge.
- Verify real streaming coding output, authentication, disconnect/cancellation,
  and absence of Reference local runner/remote fallback.
- Test tool-calling workflows only with a model/template that advertises the
  capability; Talvik remains a model provider, not a workstation agent.

## Dependencies

WP-0067, WP-0091; ADR-0013, ADR-0025.

## Acceptance criteria

- A real supported IDE session streams a meaningful coding response from
  Talvik, with logs proving the Talvik endpoint was used.
- Authentication success and failure are both evidenced.
- Client cancellation/disconnect does not corrupt subsequent inference.
- Unsupported Codex/IDE provider paths are documented as NOT VERIFIED rather
  than simulated or claimed PASS.

## Status

Configuration and API preflight assets are implemented. Interactive external
qualification remains blocked in the current execution environment and is
owner validation, not a simulated PASS.

Evidence:

- `docs/qualification/continue-talvik-config.yaml.example`
- `docs/qualification/continue-talvik-sample/`
- `scripts/qualify-talvik-openai-edge.sh`
- `console/src/bin/http.rs` request correlation diagnostics
- `docs/TALVIK-IDE-COMPATIBILITY.md`

The current host reports VS Code 1.142.0 and Continue 2.0.0. Starting the
daemon for the preflight was attempted with the configured loopback listener,
but the restricted execution environment returned `Operation not permitted`
when creating the listener. No real Continue PASS is claimed.
