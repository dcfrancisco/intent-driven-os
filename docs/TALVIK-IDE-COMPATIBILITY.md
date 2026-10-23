# Talvik IDE Compatibility and Qualification

Status: procedure prepared; real external-client qualification is **BLOCKED / NOT VERIFIED**
in the current environment.

Talvik exposes an authenticated OpenAI-compatible endpoint. The IDE is the
client; Talvik remains an inference provider and does not execute workstation
tools.

## Continue on VS Code

Configure a registered coding GGUF model using Continue's OpenAI-compatible
provider:

```yaml
models:
  - name: talvik-local
    provider: openai
    model: <registered-talvik-model-id>
    apiBase: http://127.0.0.1:<configured-port>/v1
    apiKey: <Talvik inference token>
```

Qualification:

1. Start `talvikd`; record `talvikd --version --verbose`.
2. Run `talvik --json version` and `talvik status --json`.
3. Verify `/v1/models` succeeds with the token and fails without it.
4. Select the model and request a small code change; verify streaming.
5. Confirm no Reference local runner process or remote provider is configured or contacted.
6. Cancel a request, then issue another and verify recovery.

Record IDE versions, model ID, GGUF SHA-256, endpoint, token scope (never the
token), Talvik identity, timestamps, and redacted daemon/client logs.

## Cline

Use Cline's OpenAI-compatible custom endpoint and a model whose embedded chat
template supports tools. Verify coding output, streaming, tool-call/tool-result
round trip, invalid authentication, cancellation, recovery, and absence of
Reference local runner or remote fallback. Tool support must be rejected for unsupported
templates.

## Codex

Test only if the installed version documents a custom OpenAI-compatible provider.
Do not modify Codex or infer support from HTTP tests. Otherwise record CAP-024
as deferred by client capability.

Cross-compilation is not native runtime evidence. Capture endpoint/status/log
evidence for every failed or blocked client attempt.
