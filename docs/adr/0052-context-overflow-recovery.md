# ADR 0052: A Context Overflow Is Answered by Summarizing, Once

Status: Accepted

Date: 2026-10-17

## Context

`context-compact` (plugin-gaps plan, G2) summarizes old history when it grows
past `trigger_chars`. The thresholds are characters because RAH has no
tokenizer (ADR 0004). A character count only approximates what a model's
context window holds, and a local model with a small window is where the
approximation is worst: a user must guess `trigger_chars` for each model, and
a wrong guess ends the turn with an HTTP 400.

opencode distinguishes three reasons to compact: `auto` (the context is
near its limit), `manual` (the user asked) and `overflow` (the provider just
refused the context as too long). Only `overflow` has an observable fact
behind it; `auto` needs a token count RAH does not have, and `manual` needs a
command surface the frontends do not share.

## Decision

1. **`ModelError::ContextOverflow(String)`** in `rah-api`: the provider says
   the request does not fit the model's context window. The three HTTP
   providers read at most 8 KiB of the body of a 400, 413 or 422 response and
   map it when it contains one of the phrases OpenAI, Anthropic, llama.cpp
   and their imitators use (`rah_api::model::is_context_overflow`). The body
   is only matched, never repeated: the error text is fixed, so nothing the
   provider sent reaches the user or the log.
2. **A hook, `model.overflow`** (`rah_api::hooks::ModelOverflow`), takes the
   request and returns a smaller one. `rah_api::retry::stream` (ADR 0051)
   runs it when opening fails with `ContextOverflow`, sends the result once,
   at once, and does not count that as a retry. A request the hook leaves
   unchanged, or a second overflow, ends with the overflow error. The loops
   report it as an `AgentEvent::ModelRetrying` with a reason that names the
   cause and no delay.
3. **`context-compact` attaches to it.** It summarizes with the same
   machinery and cache as the normal path but with limits taken from the
   request: it keeps about a quarter of the history (at most `keep_chars`)
   and asks for the rest to be about half, whatever `trigger_chars` says. If
   the summary fails, the request is unchanged and the turn ends with the
   overflow error.
4. **Not adopted:** an `auto` switch (mounting the plugin is the switch), a
   token `buffer` (there is no token count; usage reported by providers
   arrives after the request), and `manual` compaction (a frontend command,
   not a plugin behavior).

## Consequences

- A history that outgrew the window costs one failed request and one
  summary, not the turn. A model whose window is much smaller than
  `trigger_chars` still needs `trigger_chars` lowered to avoid the failed
  request, but no longer fails because of it.
- The summarizing request can itself overflow, when the oldest history is
  larger than the window; then the summary fails and the turn ends with the
  overflow error, which names the cause.
- The session log is unchanged, as for normal compaction: the full
  conversation stays, and the summary exists only in the request.
- Other plugins can attach to `model.overflow` (for example to drop tool
  output) with the same effect and the same single attempt.
