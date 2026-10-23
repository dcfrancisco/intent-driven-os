# WP-0092: Talvik M2 IDE API Compatibility and Qualification

## Purpose

Qualify Talvik as a real local model provider for an external coding
assistant, starting with Continue and documenting Codex/Cline capability limits.

## Scope

- Provide executable Continue configuration against Talvik's OpenAI edge.
- Verify real streaming coding output, authentication, disconnect/cancellation,
  and absence of Ollama/remote fallback.
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

Owner-deferred external qualification; implementation procedure is prepared,
but no real IDE PASS is claimed.
