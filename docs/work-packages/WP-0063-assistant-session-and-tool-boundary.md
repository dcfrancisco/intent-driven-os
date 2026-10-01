# WP-0063: Assistant Session and Tool Boundary

## Purpose

Add multi-turn assistants and tools only after the model service is reliable,
with the same governed-operation guarantees as shell actions.

## Scope

Session identity/context limits, prompt assembly, structured output, tool
capability negotiation, approval requirements, tool-call evidence, and bounded
agent loops. No unrestricted autonomous execution or implicit shell access.

## Acceptance criteria

- Sessions are separate from model lifecycle and can survive client reconnect.
- Context truncation and tool budgets are explicit and observable.
- Every mutating tool call produces an OperationPlan and follows approval,
  verification, rollback, and evidence rules.
- An assistant can be disabled while the local runner and shell remain usable.

## Dependencies

WP-0037, WP-0038, WP-0045, WP-0060, WP-0062; ADR-0005, ADR-0006, ADR-0011.
