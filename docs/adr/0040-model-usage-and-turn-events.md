# ADR 0040: Model Usage and Turn Events on the Bus

Status: Accepted

Date: 2026-10-13

## Context

Diagnosing a slow or expensive session needs per-turn data: how long model
requests and tools took, which tool calls were allowed or denied, and how
many tokens the provider charged. Two things are missing:

- **Token usage.** Providers report it (OpenAI Responses in
  `response.completed`, Anthropic in `message_start` and `message_delta`),
  but `ModelEvent` has no variant for it, so it is dropped inside the
  provider plugin.
- **A way to observe turns.** A loop sends its `AgentEvent`s only into the
  `TurnStream` of whoever started the turn. A telemetry plugin cannot see
  turns started by a frontend, the scheduler or `agent.delegate`. The event
  bus carries `SessionAppended`, which has no timings, no model requests and
  no policy outcome.

Usage is provider metadata. Provider and tool metadata are untrusted and can
never grant permission (ADR 0036, `docs/SECURITY.md`).

## Decision

1. **`ModelUsage` in `rah-protocol`.** A data-only struct with optional
   `input_tokens` (including cached), `output_tokens` (including reasoning)
   and `cached_input_tokens`. A count the provider does not report is
   `None`, never zero. It serializes without the missing fields.

2. **`ModelEvent::Usage { usage }`.** A provider sends it at most once per
   request, before `Completed`, and only when it has at least one count.
   Each provider maps its own wire format inside its plugin:
   `model-responses` reads `response.usage` (`input_tokens`,
   `output_tokens`, `input_tokens_details.cached_tokens`);
   `model-anthropic` adds `cache_creation_input_tokens` and
   `cache_read_input_tokens` to `input_tokens`, reports cache reads as
   `cached_input_tokens`, and takes the last cumulative `output_tokens`.
   `model-mock` reports an estimate (one token per four bytes) so that
   consumers can be tested without a live model.

3. **`AgentEvent::ModelUsage { session_id, model_request_id, usage }`.**
   Loops forward the provider's usage for the model request it belongs to.

4. **`TurnEvent` on the bus.** `rah-api::agent_loop::TurnEvent { event }`
   is published on the kernel event bus for every event a loop puts into a
   `TurnStream`. It is an observation copy: the turn neither waits for nor
   depends on subscribers, and a subscriber cannot change it. Both loops
   (`loop-react`, `loop-plan`) publish it; a new loop should too.

5. **`telemetry-log`** is the first subscriber. It is opt-in, writes JSONL
   records of turns, model requests and tools to files it rotates and
   prunes, and never records message text or tool output; tool arguments
   only with `include_arguments`.

6. Usage is for display and records only. No policy, budget or loop
   decision reads it in this ADR; a future budget plugin must treat it as
   provider-reported and possibly missing.

## Consequences

- Adding variants to `ModelEvent` and `AgentEvent` breaks exhaustive
  matches in Rust consumers; in-tree ones are updated. JSON consumers that
  ignore unknown `type`s (the Web UI) keep working. Process and WASM
  plugins do not receive these events, so their protocols are unchanged.
- `ModelDelta` events now also go through the bus. Handlers run
  synchronously in `emit`, so subscribers must be cheap and hand slow work
  to a scoped task (as `telemetry-log` does with a channel).
- The Web UI shows the tokens reported for the open session since the page
  was loaded. Usage is not written to the session log, so a reload starts
  the count again; `telemetry-log` keeps the durable record.
- Providers that do not report usage (or a local server that omits it) send
  no `Usage` event, and consumers show nothing rather than zero.
