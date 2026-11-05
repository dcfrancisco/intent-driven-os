# MarinaOS desktop screen and state specification

```mermaid
flowchart LR
  Start[Desktop starts] --> Health{Foundation IPC}
  Health -->|connected| Overview[Overview + measured hardware]
  Health -->|unavailable| Offline[Offline / reconnect guidance]
  Overview --> Request[Request entered]
  Request --> Contract[Governed intent API]
  Contract -->|not exposed| Unavailable[No execution claimed]
  Contract -->|future| Plan[Plan -> approval -> execution -> verification -> evidence]
```

The current UI distinguishes connected foundation data, unavailable foundation,
optional Talvik, request-not-submitted, and IPC-receipt timeline states. Future
governed states (`pending`, `planned`, `awaiting approval`, `approved`,
`executing`, `verifying`, `verified`, `rejected`, `failed`) must be driven by
backend events.
