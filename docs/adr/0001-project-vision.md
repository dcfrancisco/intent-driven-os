# ADR-0001: Project Vision

- Status: Accepted

## Context

The project needs a distinctive first release that demonstrates its value without attempting to build an entire intent-driven operating system at once. The most immediate architectural opportunity is to pair a model-management service with a focused, text-first client.

## Decision

Define the project as an AI-native runtime and console for Linux, developed in two initial phases:

1. **Intelligent Runtime:** a systemd-like control plane for models. It manages installed models, loading and unloading, hardware detection, CPU/GPU/NPU allocation, backend adapters, streaming, security, logging, and health monitoring. It has no UI.
2. **AI Console:** a keyboard-first, retro terminal-style client. It is not another chatbot or conventional terminal; its prompt expresses user intent and presents streaming, safe, auditable results.

Desktop integration, background services, broader OS integration, and enterprise integrations follow these foundations rather than preceding them.

## Consequences

The first demonstrable system has a narrow, coherent scope and a clear separation between intelligence services and presentation. The runtime can serve multiple clients, while the console can evolve independently. The project must invest early in stable contracts, lifecycle management, security, and streaming UX rather than only model inference.

## Alternatives Considered

- Building the complete intent-driven OS first would delay a usable demonstration and create too many coupled decisions.
- Building only a chatbot would not establish model resource management or intent-based system operations.
- Building only a traditional terminal would not demonstrate the AI-native interaction model.

## Future Considerations

Future clients may include a desktop, REST API consumers, background services, local applications, and OIP enterprise integrations. The console may eventually blend shell commands, AI-assisted operations, development workflows, administration, and model management while preserving explicit policy and audit boundaries.

