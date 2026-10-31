# VS Code Talvik presets

The native OpenAI-compatible endpoint in the local qualification is `http://127.0.0.1:11435/v1`. When VS Code asks for a full Chat Completions URL, use `http://127.0.0.1:11435/v1/chat/completions`. Store the private API token through VS Code's secret picker. Do not put its value in model examples, agent files or reports.

## Select the matching model

| Mode / preset | Configured display name | Client capability |
| --- | --- | --- |
| Talvik Coding | `Talvik Qwen3.5 4B (Agent, Chat, Plan, Vision)` | Tools, streaming and vision, with matching projector |
| Talvik Plan | `Talvik Qwen3 1.7B (Chat, read-only Plan)` | Tools and streaming; text-only; preset restricts tools to reads/search |
| Lightweight Ask | Local Qwen2.5 Coder 1.5B chat entry | Tools disabled in the tested configuration |

These names must exactly match the customendpoint model display names in your VS Code configuration. The agent `model` value appends ` (customendpoint)` as the vendor identifier. The display names describe intended usage, not a guarantee of qualification. Read the current [status and model limits](../../reports/TALVIK-STATUS-2026-11-01.md).

Import/load the corresponding compatible GGUF in Talvik. For the local 32K daemon setup, the recorded client budgets are 28,672 input and 2,048 output tokens. A large declared limit does not make cold prefill fast. Enable tools only when the selected model's runtime profile supports them; enable vision only with a matching usable projector. Set `modelOptions.chat_template_kwargs.enable_thinking` to `false` only for a profile supporting that control. The current Qwen3.5 candidate uses temperature 0 and top-p 0.95 for qualification.

## Install and use the presets

Copy [talvik-coding.agent.md](talvik-coding.agent.md) and [talvik-plan.agent.md](talvik-plan.agent.md) into VS Code's user prompts directory. On the qualification Mac it is `~/Library/Application Support/Code/User/prompts`. Select **Talvik Coding** or **Talvik Plan** in a fresh conversation. These presets narrow tool availability and instructions; they cannot repair a broken server formatter or make an unsuitable model reliable.

Talvik Coding allows file/search/edit/terminal tools. For a known Maven project, build-only instructions request `mvn -DskipTests package` and exclude inspections, edits, cleaning, installation and tests. Talvik Plan is read-only and intentionally cannot build. Ask should answer self-contained questions without unnecessary tools.

## Acceptance boundary

The `5d5925b` native template integration is committed and unit/native-tested; installation and a new full VS Code acceptance run remain pending. Earlier runs include narration without execution and a real `mvn clean install` that violated build-only scope. A successful API tool probe does not prove IDE execution. Follow the [manual runbook](../../reports/TALVIK-QUALIFICATION-RUNBOOK.md), capture actual terminal results, and retain the current RC blockers until that evidence passes.
