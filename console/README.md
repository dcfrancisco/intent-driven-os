# AI Console Crate

Phase 2 interactive AI Console boundaries:

- `tui` — application lifecycle
- `prompt` — keyboard prompt presentation
- `renderer` — terminal rendering
- `history` — command history boundary
- `commands` — intent command boundary
- `editor` — keyboard editing and terminal input
- `presence` — runtime-event-driven adaptive cursor state
- `status_bar` — deterministic runtime status presentation

The executable provides an interactive prompt, history navigation, editing keys,
placeholder commands, a status bar, and a mock runtime. It does not integrate
an AI model, llama.cpp, hardware probing, or Linux operations.

Phase 3 runtime commands include `backend`, `backend list`, `backend status`,
`models`, `model inspect <id>`, `hardware`, `runtime`, and `health`. All data
comes through `RuntimeService`; the console never accesses adapters directly.
## Governed operations

The interactive console owns one persistent operation coordinator for the
session. Mutating commands render a plan before execution, and explicit
recovery controls are available through:

```text
operations recover
operations approve <operation-id>
operations rollback <operation-id>
```

Approval records, operation state, and evidence are stored in the OID console
state directory under the system temporary directory.

Ordinary input is now executed through the configured shell. OID controls use
the `:` namespace, for example `:help`, `:status`, and `:intent <description>`.
