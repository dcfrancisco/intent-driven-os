> Historical snapshot imported on 2026-11-01. Build identities, pass claims, pending work, installed versions, and push status below describe their original run, not the latest source. See the [current status report](../TALVIK-STATUS-2026-11-01.md). Local-only archive/image references may require the original artifact directory.

# Talvik API stabilization report — 2026-10-31

> Current installed build: `a480958`. See [loaded profiles and output adapters](TALVIK-LOADED-PROFILES-REPORT-2026-10-31.md) for current behavior and exact test status. Earlier build identities and measurements below are historical.

> Current installed build: `6f3f9bd`. See [model adaptation implementation](TALVIK-MODEL-ADAPTATION-STUDY-2026-10-31.md) for current qualification. Measurements and package identities below remain evidence for their stated earlier commits.

> Historical candidate: installed binaries now use latency build `8c64517`. See [latency investigation](TALVIK-LATENCY-REPORT-2026-10-31.md) for current installed identity and measurements. The archive below remains the older `e91dc0f` build.

Status: **local Intel macOS CPU API candidate; not an RC tag or cross-platform release.**

The API recovery and interoperability gates now have direct executable evidence.
The remaining IDE and release qualifications are listed below rather than hidden
behind an API success claim.

## Candidate and installed build

- Branch: `feature/qwen-coding-assistants`.
- API hardening commit: `d24878103262334c461d7c971805f051cbf13734`.
- Compiler/CI commit: `77a0c28`.
- Final installed/packaged readiness build: `e91dc0f994f83eebe7a568593ec60889dfc38f30` (clean source; Rust **1.88.0**).
- All four installed clients/daemon were rebuilt together. The fresh archive
  includes the standalone `talvik` and `talvikd` payload with native libraries.
- Endpoint: `http://127.0.0.1:11435/v1`; existing private bearer token retained.
- Native profile: pinned llama.cpp, CPU + Apple BLAS; **no GPU qualification**.

## Fixed

- `/readyz` and `/healthz` inference readiness now follow actual model residency,
  including manual load/unload when default-model auto-load is disabled.

- A disconnected pre-token SSE request used to continue consuming the model.
  Waiting handlers now detect closed sockets every 100 ms, cancel their work,
  and wait for native cleanup. OpenAI and Ollama generation edges share cleanup.
- Transport errors, including failure to write initial headers, cannot orphan
  inference or cancellation ownership. Socket writes have a five-second timeout.
- Duplicate active/queued request IDs cannot replace another caller's ownership.
- Unknown models return 404; invalid context overflow retains its request-error
  type and returns 400. Admission and backend failures use distinct statuses.
- Fixed the release toolchain mismatch: locked image dependencies require Rust
  1.88, while the project/CI previously declared 1.82. Both now use 1.88.
- The release check job now tests the full locked workspace, including the
  daemon HTTP tests. Ownership/assertion lint errors were corrected; narrow
  serializer-size/argument allowances are documented locally, not applied to
  the entire workspace.

## Results

| Check | Result | Evidence |
|---|---|---|
| Rust 1.88 workspace | **262 passed**, 5 fixture tests ignored by default | [Workspace log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-workspace-tests.log>) |
| Strict release Clippy | **Passed** on Rust 1.88 | [Lint log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-strict-clippy.log>) |
| Formatting / diff whitespace | Passed | Local `cargo +1.88.0 fmt --all -- --check`, `git diff --check` |
| Previously ignored real-GGUF fixtures | **All 5 passed when explicitly invoked** | [Text/grammar log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-native-fixtures.log>), [vision log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-vision-fixture.log>) |
| Native API chat/coding/vision/tool/result | **5 passed** on final installed build | [JSON](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-native-api-report.json>) |
| Live API validation/failure/recovery | **14 passed** on final installed build | [JSON](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-api-recovery-report.json>) |
| Official Python OpenAI SDK 2.48.0 | Discovery and **6 generation shapes passed** | [SDK log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-openai-sdk.log>) |
| Installed CLI terminal behavior | **6 passed** | [CLI terminal log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-cli-terminal.log>) |
| Installed OID console terminal behavior | **7 passed** | [Console terminal log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-console-terminal.log>) |
| Isolated archive install/status/uninstall | **Passed**; no developer library paths; state preserved | [Install log](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-rc-install-smoke.log>) |
| Managed forced-crash recovery | **Passed**, ready in 2.83 s and next inference succeeded | [Restart JSON](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-managed-restart-report.json>); final installed readiness build |
| Manual model lifecycle + native budget recovery | **Passed**: 503→200→503→200; context rejected with 429; twelve subsequent completions | [Resource profile](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-resource-profile-report.json>) |
| Actual VS Code Agent read/tool/result cycle | **Failed latency budget**: no answer/tool result after 350.76 s; cancelled; subsequent API request succeeded | [Run record](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-ide-agent-check.json>) |

The 14 API cases cover authentication, required model, unknown model, unsupported
options, invalid output/context limits, SSE output-limit finish reason and usage,
prompt overflow then recovery, non-stream and stream deadline recovery,
disconnect before and after the first token, Ollama disconnect recovery, full
queue rejection, duplicate-ID rejection and queued cancellation/recovery. Error
cases verify subsequent generation succeeds rather than only checking health.

SDK shapes: streamed and non-streamed Chat Completions, legacy Completions and
text Responses. The native tool/result test uses a synthetic client-supplied
tool result; it does not execute a workspace tool. Real fixture tests separately
exercise native grammar and vision cancellation/recovery.

## Remaining before RC promotion

1. **IDE Agent latency and Plan acceptance.** The real Agent request used Qwen3.5
   4B and **22,107 prompt tokens**. No tool call or final marker appeared within
   the five-minute response budget; the request was cancelled after **350.76 s**.
   The next short API request worked. Actual Plan is still unqualified. These
   modes must not be advertised as qualified on this Intel CPU profile.
2. **Publishing and other-platform qualification.** GitHub push/remote CI remain
   blocked by missing authentication; the SSH agent also has no identities.
   Clean-machine install/upgrade evidence is still needed for each advertised
   platform. The local isolated archive install is not a separate clean machine.
3. **Broader compatibility and extended load.** Only the documented route/client
   subset is qualified. Other SDKs, embedding-model profiles and longer mixed
   client runs need their own evidence. Controlled native admission-budget and
   serial recovery now pass; they do not simulate OS-wide memory exhaustion.

Native API acceptance on the final build:

| Case | Seconds | Result |
|---|---:|---|
| chat | 17.88 | Pass |
| coder | 8.48 | Pass |
| vision | 4.9 | Pass |
| agent_tool_sse | 17.77 | Pass |
| agent_tool_result | 22.53 | Pass |

The archive is an API stabilization preview, not an RC announcement. Existing
working Ask evidence remains valid but does not prove Agent/Plan. This host's
Intel GPU remains unqualified; enabling it previously produced incorrect output.

## Local artifact

[Build identity record](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-api-build-identity.json>): source, installed CLI/daemon, HTTP and archive all match the clean final commit.

SHA-256: `c68d0cfa8ff17c0a9bc6b7480ac4b04fe3390fea06d1d600aded1a07465deef2`.

- [Preview archive](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-api-stabilization-macos-intel.tar.gz>)
- [SHA-256](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/talvik-api-stabilization-macos-intel.tar.gz.sha256>)

No token or model weights are included in the archive or source commits.

## Quick checks

```sh
curl http://127.0.0.1:11435/readyz
~/.marina/bin/talvik status
```

For the full idle-daemon recovery suite, run the committed
`scripts/test-talvik-api-recovery.py`. It reads the existing private token file
automatically and cancels only its own test request IDs. The native vision/tool
suite is `scripts/test-talvik-native-api.py`; the resource fixture is
`scripts/test-talvik-resource-profile.py`. Keep these bounded tests separate
from active IDE requests. Overnight recurring tests remain paused.
