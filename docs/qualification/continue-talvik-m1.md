# Continue + Talvik M1 IDE qualification

This is the reproducible Continue procedure. It uses a temporary Continue
global directory and does not modify `$HOME/.continue` or `$HOME/.marina`.

Create `/tmp/talvik-continue-m1/config.yaml` with:

```yaml
name: Talvik M1 Qualification
version: 1.0.0
schema: v1
models:
  - name: Talvik Qwen
    provider: openai
    apiBase: http://127.0.0.1:18082/v1
    apiKey: local-test-key
    model: qwen2.5-0.5b-instruct-q4_k_m
    contextLength: 4096
    completionOptions:
      temperature: 0
```

Start Talvik with `TALVIK_HTTP_AUTH=false`, then launch an isolated VS Code
window:

```sh
CONTINUE_GLOBAL_DIR=/tmp/talvik-continue-m1 \
code --user-data-dir /tmp/talvik-vscode-m1 \
  --extensions-dir "$HOME/.vscode/extensions" --new-window "$PWD"
```

In Continue, select `Talvik Qwen`, submit:

```text
Write a Python function named add that returns the sum of two numbers. Reply with only the function.
```

Expected evidence is a relevant function response, a daemon HTTP request in
the captured log, no Reference local runner process/request, and no remote endpoint in the
temporary config. Capture VS Code/Continue version, config hash, daemon log,
prompt/response transcript, and process/network evidence. For Cline, repeat
with a tool-using coding task only after a real Cline extension/session is
available; tool execution must remain explicitly bounded and audited.

The 2026-10-23 host had Continue 2.0.0 and VS Code 1.142.0, but macOS
Accessibility denied automated UI keystrokes with AppleScript error 1002. No
authenticated Continue or Cline request was therefore verified, and the IDE
gate remains NOT VERIFIED.
