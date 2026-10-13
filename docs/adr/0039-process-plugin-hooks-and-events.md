# ADR 0039: Hooks and Events for Process Plugins

Status: Accepted

Date: 2026-10-13

## Context

A process plugin (protocol v2) can only provide `tools`. Policy, output
post-processing and context plugins attach to hooks (`tool.before_invoke`,
`tool.after_invoke`, `model.before_request`), and indexing or title plugins
follow `SessionAppended` events. All of these can currently be written only
in Rust. `docs/PROCESS_PLUGINS.md` lists hooks and events as not yet
available.

A hook differs from a tool in what a failure means:

- `tool.before_invoke` is an authorization chain. A handler that cannot
  answer must not let a call through.
- A static Rust handler can fail only by panicking. The kernel contains the
  panic and returns `HookError::Panicked`. Callers treat that as a denial
  (`tool.before_invoke`), an error output (`tool.after_invoke`) or a failed
  model request (`model.before_request`).
- A process handler can also time out, disconnect or return a malformed
  answer.

A v2 host ignores manifest fields it does not know. Adding `hooks` to
protocol v2 would let a policy plugin mounted on an older `rah` start without
guarding anything.

## Decision

1. **Protocol 3.** The host speaks protocol 3 (`initialize.protocol` and
   `RAH_PLUGIN_PROTOCOL` are `3`). It still accepts a manifest that answers
   protocol `2`, which may only provide tools as before. `hooks` and
   `events` need protocol `3`. A v3 plugin on an older host is refused
   ("the plugin speaks protocol 3"), so it fails closed.

2. **Manifest.** Two optional fields:
   - `hooks`: `[{"hook": "<name>", "priority": <i32>}]`, at most one entry
     per hook. Only `tool.before_invoke`, `tool.after_invoke` and
     `model.before_request` are allowed. `priority` defaults to `0`
     (`policy-authority` uses `0`; higher runs first).
   - `events`: `["session.appended"]`, the only event offered.
   Both take part in the check that a restarted plugin's manifest is
   unchanged.

3. **Hook calls.** The host sends the request `hooks/call {hook, input}`.
   It is bounded by `hook_timeout_ms` (default 5000) and cancelled with the
   plugin scope.
   - `tool.before_invoke`: `input` is `{session, call, definition}`. The
     result is `null` (no decision; the next handler runs) or
     `{"decision": "allow"}`, `{"decision": "deny", "reason"}` or
     `{"decision": "ask", "reason"}`.
   - `tool.after_invoke`: `input` is `{session, call, definition, output,
     effect}`. The result is `null` (unchanged) or `{"output": <tool
     output>}`, checked like a `tools/call` result (including images).
     Only the output can change.
   - `model.before_request`: `input` is the model request. The result is
     `null` (unchanged) or `{"request": <model request>}`. Images in
     returned tool results are checked as in tool output.
   `call.input` and the request's context come from the model and are
   untrusted. The plugin itself is trusted, as every mounted plugin is:
   mounting it is the user's decision. It can therefore allow calls that
   `policy-authority` would deny, if its manifest gives it a higher
   priority. A Rust plugin on the same hook can do the same.

4. **Failures behave like a failing static handler.** A timeout,
   cancellation, disconnect, error response or malformed result:
   - `tool.before_invoke`: a denial whose reason names the plugin and the
     failure. The chain stops, so the call fails closed.
   - `tool.after_invoke`: the output is replaced by an error output. The
     tool's recorded effect does not change.
   - `model.before_request`: the handler fails the way a panicking handler
     does: it resumes an unwind that the kernel contains (no panic message
     is printed), and the loop fails the turn. Context or redaction
     plugins therefore do not fail open.
   Hook calls are never retried.

5. **Events.** A subscribed `SessionAppended` is sent as the notification
   `event {"event": "session.appended", "data": {session, seq, entry}}`.
   Delivery is best effort, in order, at most once. Events while the plugin
   is not running are lost, and the plugin cannot acknowledge them. Use
   `seq` and `service/call session.read` to catch up.

6. **Not in scope.** Plugins still cannot provide services other than
   `tools`, emit events, or attach to `tool.approval` or `host.interrupt`.
   No kernel or `rah-api` change is needed.

## Consequences

- A tool policy, an output filter or a context source can be written in any
  language.
- A process `tool.before_invoke` handler adds a round trip to every tool
  call, and a `model.before_request` handler adds one to every model
  request. The whole request crosses the pipe, so `max_message_bytes` must
  hold the largest context.
- A `model.before_request` failure is reported by the kernel as a handler
  panic (`HookError` has no other failure variant).
- Events have no backpressure. A plugin that stops reading stdin blocks its
  own connection, as it already does for tool calls.
