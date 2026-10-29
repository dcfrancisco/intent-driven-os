# Talvik IDE compatibility

Status: Windows x64 OpenAI SDK/API subset passed with Qwen 2.5 0.5B;
interactive Continue qualification remains NOT VERIFIED, and Cline was not
installed.

## Windows SDK/API qualification — 2026-10-29

The official OpenAI Python SDK 3.41.0 and JavaScript SDK 7.45.1 passed
model discovery, streaming and non-streaming Chat Completions and legacy
Completions, text Responses, tool-choice modes, and a tool-call/result-message
round trip against a loopback release daemon using Qwen 2.5 0.5B Q4_K_M.
Automatic tool-call SSE and an 896-dimensional embedding response also passed.
A planning-style request used Chat Completions; Talvik has no separate Plan
endpoint. Responses streaming returned the expected HTTP 501.

The daemon was loopback-only and HTTP authentication was disabled. Continue
2.0.0 was installed but no interactive VS Code/Continue session was run, so
these API tests do not qualify IDE integration. Cline was not installed, and
Talvik did not execute the transported tool calls.

## Host and daemon contract

- VS Code: 1.142.0, x64, commit `6d62b1d5dace93a33b3a6d3eca1317bd5e15bcb1`
- Continue extension: 2.0.0, `continue.continue-2.0.0-darwin-x64`
- Listener selected by the current shell: `127.0.0.1:11434`
- API base for Continue: `http://127.0.0.1:11434/v1`
- Authentication: bearer token required by default (`TALVIK_HTTP_AUTH=true`)
- Registered coding model: `deepseek-coder-1.3b`
- Alternate registered model: `qwen2.5-0.5b-instruct-q4_k_m`
- Persistent state and models: `$HOME/.marina`
- DeepSeek GGUF SHA-256: `d040cc18521592f70c199396aeaa44cdc40224079156dc09d4283d745d9dc5fd`
- Qwen GGUF SHA-256: `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`
- Release CLI SHA-256: `c289cfe8a3caf86946817fb0e3602c39e49a04a3c63cf40485b539829bfb0eb0`
- Release daemon SHA-256: `cb231e8ec800c5819b1995467120bc269dabd48e3f853f8ce0a0b4604e3e21f7`

The shell currently has `TALVIK_HTTP_ADDR=127.0.0.1:11434` and
`TALVIK_HTTP_AUTH=true`. An explicit environment override or config file can
change those values; record the effective values in the qualification log.

## Additive Continue configuration

Use [continue-talvik-config.yaml.example](qualification/continue-talvik-config.yaml.example)
as a separate Talvik-specific configuration. It does not modify the existing
`$HOME/.continue/config.yaml`. Continue 2.0.0's installed YAML schema accepts
the `openai` provider, `apiBase`, `model`, and `apiKey` fields. The example
contains only a placeholder; insert a token locally and never commit the
completed file.

Create a scoped token without exposing it in this repository:

```sh
export TALVIK_CONTINUE_TOKEN="$($HOME/.marina/bin/talvik auth token create continue-local)"
```

Copy the example to a private config location, replace
`REPLACE_WITH_TALVIK_TOKEN`, and select `Talvik DeepSeek Coder` in Continue.
The installed Continue schema stores `apiKey` as a literal string; do not put
an unexpanded shell variable in the YAML and assume it will be resolved.

## API preflight

With `talvikd` running on the selected loopback address, run:

```sh
TALVIK_CONTINUE_TOKEN="$TALVIK_CONTINUE_TOKEN" \
  TALVIK_OPENAI_BASE_URL=http://127.0.0.1:11434/v1 \
  TALVIK_MODEL_ID=deepseek-coder-1.3b \
  scripts/qualify-talvik-openai-edge.sh 2>&1 | tee /tmp/talvik-continue-api.log
```

This checks model discovery, non-streaming coding output, SSE streaming with a
terminal `[DONE]`, and the model's tool-call capability. A `501` tool response
means the selected GGUF template does not support tools and is an explicit
capability result, not a successful tool qualification.

Authentication failure must also be checked without printing the valid token:

```sh
curl -i -sS -H 'Authorization: Bearer invalid' \
  http://127.0.0.1:11434/v1/models
```

Expected status: `401`.

## Interactive qualification

Open `docs/qualification/continue-talvik-sample` as a small workspace. Submit
the three prompts in its README, verify streamed coding output, cancel one
request, and submit the next prompt. Capture the Continue/VS Code version, the
selected model, the config hash with the token redacted, and the Talvik log.

Talvik logs only request ID, model ID, stream/tool flags, terminal state, and
token count. Correlate those lines with the Continue request; prompts and
responses are not logged by default.

Expected-results checklist:

- [ ] Continue selects `Talvik DeepSeek Coder`.
- [ ] `/v1/models` lists the selected model.
- [ ] A coding response streams into Continue.
- [ ] The Talvik log contains matching `request_id` and model lines.
- [ ] Cancellation is followed by a successful new request.
- [ ] Invalid authentication is rejected.
- [ ] No alternate provider is configured or contacted.
- [ ] Tool calls are either completed with a capable template or explicitly
      rejected with the documented capability response.

## Actual evidence status

The source build and version identity were verified on this host at commit
`bfea51e47536435f040756fd4ef4dd595b94a6e1`. The daemon was started with:

```sh
TALVIK_HTTP_ADDR=127.0.0.1:11434 TALVIK_HTTP_AUTH=true \
  $HOME/.marina/bin/talvikd
```

During that macOS-host qualification attempt, the restricted execution
environment returned `Operation not permitted` while creating the loopback listener. Therefore model listing,
chat, streaming, and tool-call HTTP results are NOT EXECUTED here, and no
interactive Continue PASS is claimed. The owner can run the commands above in
the normal user session and attach `/tmp/talvik-continue-api.log` plus the
daemon stderr log.
