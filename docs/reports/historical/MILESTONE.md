> Historical snapshot imported on 2026-11-01. Build identities, pass claims, pending work, installed versions, and push status below describe their original run, not the latest source. See the [current status report](../TALVIK-STATUS-2026-11-01.md). Local-only archive/image references may require the original artifact directory.

# Talvik M2.1 — local stabilization preview

Latest action repair: installed `f3d51b0`, with test script through `0b63532` — [action follow-through report](TALVIK-ACTION-FIX-2026-11-01.md). Live API action checks pass; actual IDE build execution is being qualified separately. Earlier identities below are historical.

Previous installation: `37d44a1` — [VS Code fixes and qualification](TALVIK-VSCODE-FIX-REPORT-2026-11-01.md). Earlier build identities, push status and measurements below are historical. Cold CPU latency remains a release blocker.

Historical profile/parser build: `a480958` — [implementation and tests](TALVIK-LOADED-PROFILES-REPORT-2026-10-31.md). Profiles, selected tool protocols and separated reasoning transport are implemented. Feature acceptance passed; final idle-only regression checks were interrupted by a new user IDE request. Earlier entries below are historical.

Historical model-adaptation build: `6f3f9bd` — [implementation and tests](TALVIK-MODEL-ADAPTATION-STUDY-2026-10-31.md). Template-derived checkpoint boundaries are implemented and qualified locally. Capability profiles, parser selection and full IDE/RC qualification remain open. Earlier milestone entries below are historical.

Historical latency build: `8c64517` — [latency investigation](TALVIK-LATENCY-REPORT-2026-10-31.md). Warm recurrent-cache acceptance passed; cold Agent/Plan and reference-runner parity remain open.

Previous API stabilization build: `e91dc0f` — [historical API test report](</Users/abigailbangasan/Documents/ChatGPT/Model Runner/TALVIK-API-RC-REPORT-2026-10-31.md>). API recovery/native checks pass; Agent latency and remote qualification still block RC. Earlier milestone evidence below is historical.

Built and installed on 2026-10-31. This is a working local runner milestone,
not a release candidate or production-readiness claim.

Source: `7c3a5edf8bb7780ec0f607af3340ed72251e3767`, clean build, branch
`feature/qwen-coding-assistants`. The CLI, daemon, HTTP identity and packaged
binaries match. The branch is committed locally; push failed because GitHub
HTTPS credentials are unavailable.

## Working and verified

- One managed `talvikd`, shared CLI/API registry and socket, ready at
  `http://127.0.0.1:11435`. OpenAI base URL: `http://127.0.0.1:11435/v1`.
- Native Qwen Chat, Qwen2.5 Coder, Qwen3.5 vision, standard SSE including usage,
  required tool arguments and correlated tool-result continuation.
- No trailing `<` or empty thinking wrapper in the final five live API checks.
- Concurrent requests queue instead of racing; 64-request regression passes.
- Active socket protection, stale socket recovery, duplicate HTTP startup
  failure, cancellation followed by the next request.
- Real VS Code Ask completed on this build in about 47 seconds. Its Qwen Coder
  utility request also completed. That utility duration included queue wait.
- macOS archive installs without a checkout or developer library paths,
  starts, answers CLI status, stops, and uninstalls while preserving models.
- 229 workspace tests passed, 0 failed, 5 ignored; formatting passed; five
  installer contract tests passed. Strict Clippy still fails; see its log.
- CPU Apple BLAS prompt processing measured 1.33x faster on the same 769-token
  prompt (33.9s to 25.4s), with the same correct response.
- Last night's recurring soak test is paused.

## Use this installed build

```sh
~/.marina/bin/talvik status
~/.marina/bin/talvik models
~/.marina/bin/talvik run qwen2.5-coder-3b-instruct-q4_k_m 'Write a Python clamp function.'
curl http://127.0.0.1:11435/readyz
```

VS Code's existing Talvik endpoint and private API token are configured. Start a
fresh Ask conversation for a basic test. The real Agent tool loop and Plan mode
remain unqualified; large Agent prompts contain about 22K tokens and are too
slow on this Intel CPU profile. A successful direct API tool test alone does
not prove IDE Agent compatibility.

Run bounded API acceptance again, when needed (not scheduled):

```sh
cd /Users/abigailbangasan/codes/intent-driven-os
TALVIK_TEST_REPORT=/tmp/talvik-api-report.json python3 scripts/test-talvik-native-api.py
```

The test uses the existing private token file without printing it and exercises
synthetic tool results; it does not execute a filesystem tool.

## Artifacts and RC blockers

`talvik-m2.1-preview-macos-intel.tar.gz` and its `.sha256` are the local CPU
preview installer. Models and tokens are not bundled. Build identity, native
revision and per-test evidence are in `talvik-milestone-qualification.json` and
`talvik-build-identity.json`; full workspace, archive and strict-lint logs are
beside this file.

RC promotion requires actual Agent/Plan acceptance, acceptable large-prompt
latency, strict Clippy cleanup, cross-platform clean-machine installation and
upgrade, compatibility inventory, and recovery qualification. The Intel Iris
Vulkan path produced incorrect output and is not a qualified GPU profile.
No RC tag or public release was created.


## CLI presentation follow-up

The installed `talvik` and `oid-marina-chat` clients now use clean commit
`7d775f3a7c53bae0447030625f32570e9a4c1c39`. The API daemon was kept running
on the previously qualified build; this client update did not replace the
older milestone archive above.

Terminal output now includes elapsed waiting progress, Thinking/Generating
sections, colors, lightweight Markdown/code highlighting, and compact metrics.
Empty thinking sections are hidden. Cancellation, output limits and context
limits have distinct outcomes. Piped output stays raw; `--plain`, `TALVIK_PLAIN=1`,
`NO_COLOR`, and `TERM=dumb` are supported.

255 workspace tests passed (5 ignored); strict lint passed for all three changed
client binaries; all six pseudo-terminal acceptance tests passed against the
installed binaries without developer native-library overrides. The final
console tests and formatting checks passed. The live queued demonstration
showed waiting time and cancelled promptly without cancelling the active IDE
request. Full daemon lint/Agent/Plan RC blockers listed above remain open.
