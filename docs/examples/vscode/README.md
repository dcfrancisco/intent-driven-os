# VS Code CPU preset

`talvik-coding.agent.md` provides a focused coding mode with file, search,
editing, and terminal tools. It expects a customendpoint model whose display
name is `Talvik Qwen3 1.7B (Agent, Plan)`. The `(customendpoint)` suffix in the
agent's model field is the vendor identifier required by VS Code.

Import and load a compatible Qwen3 1.7B GGUF, configure the endpoint as
`http://127.0.0.1:11435/v1/chat/completions`, enable tool calling and streaming,
and disable vision for this text model. Use a context budget supported by the
daemon; the local 32K setup uses 28,672 input and 2,048 output tokens. Set
`modelOptions.chat_template_kwargs.enable_thinking` to `false` for interactive
latency when the loaded profile reports thinking control.

Copy the agent files into VS Code's user prompts directory and choose
**Talvik Coding** or the read-only **Talvik Plan** preset. Existing model names and ports can be substituted.
Use the secret picker for API credentials; keep tokens out of configuration
examples and source control.

A smaller chat-only model can be selected independently in Ask mode. In the
local qualification, Qwen2.5 Coder 1.5B passed simple chat but failed tool-result
continuation, so its client tool-calling capability is disabled. Qwen3.5 4B
remains available separately for vision. These are tested local presets, not
a guarantee of arbitrary coding tasks or low cold-start latency on every CPU.
