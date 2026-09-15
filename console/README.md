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
placeholder commands, a status bar, and Marina's in-process runtime service.
The runtime owns AI model, llama.cpp, hardware, and Linux-operation boundaries.

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

## System audio

On Linux, OID can control the default system output through PipeWire/WirePlumber
or PulseAudio. Set the output gain with `:audio 1x`, `:audio 2x`, or
`:audio 3x`. Use `:audio status`, `:audio mute`, `:audio unmute`, and
`:audio toggle-mute` for the related controls. Gain above 1x can clip or distort
audio; `:audio reset` restores 100%.

On macOS, 2x/3x uses the BlackHole 16ch virtual device and FFmpeg's
AudioToolbox output. Install BlackHole 16ch and FFmpeg, reboot macOS so the
driver is registered, then use `:audio 2x` or `:audio 3x`. `:audio reset`
stops the relay and restores the previous physical output.
