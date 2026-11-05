# MarinaOS desktop components

```mermaid
flowchart TB
  GTK[GTK 4 / libadwaita user-session app]
  GTK --> View[Overview, requests, timeline, settings]
  View --> Adapter[Typed UI adapters]
  Adapter --> IPC[Versioned local IPC]
  IPC --> Foundation[marinaosd: status, doctor, hardware, version]
  Adapter -. future .-> Governed[Authenticated intent/operation API]
  Governed --> Runtime[intent-runtime + policy + coordinator]
  Runtime --> Evidence[verification + evidence]
  GTK -. optional .-> Talvik[Talvik availability only]
```

Blocking IPC is isolated to a worker thread and results are delivered through
a nonblocking main-loop poll. No UI component owns privileged handles or native
model state.
