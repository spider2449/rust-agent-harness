# ADR 0045: Tool Admission Bounds Calls in Flight

Status: Accepted

Date: 2026-10-15

## Context

The legacy host refused a tool call while 128 calls were in flight. The
pivot moved authorization to `policy-authority` on the `tool.before_invoke`
hook but left the limit out (pivot plan, P4 notes): a hook decides and
returns, so it cannot tell when the call it allowed ends. A count kept by a
hook would leak whenever a call is denied later in the chain, an `Ask` is
rejected, the call is cancelled, or the invocation future is dropped.

`ToolInvoker::invoke` is the one place that sees a call from authorization to
its end, and dropping its future drops everything it holds.

## Decision

1. **`rah-api::tools::ToolAdmission`**, a service (`tool-admission`,
   `TOOL_ADMISSION`) with one method,
   `admit(&ToolRequestContext) -> Result<ToolPermit, String>`. It must not
   wait. `ToolPermit` wraps any value whose drop releases the admission.
2. **`ToolInvoker::invoke` asks every mounted provider** after the policy
   allowed the call and before the tool starts (before `on_authorized`, so a
   loop sends no `ToolStarted` for a refused call). The first refusal wins;
   permits already given are dropped. A panicking provider refuses. A
   refused call is not started: it returns `not started: <reason>`, with
   `authorized = false` and effect `none`. The permits are held until the
   tool returns or is cancelled, and are released with the invocation if its
   future is dropped.
3. **`policy-authority` provides it** with `max_in_flight` (default 128, the
   legacy limit; `0` is a configuration error): one count for all sessions.
   A call over the limit is refused, not queued, and nothing retries it.
4. **The tools key stays private.** Admission sees the request, not the
   tool, and cannot call one.

## Consequences

- The limit holds whatever way a call ends, because the permit's lifetime is
  the invocation's.
- Admission runs after `tool.before_invoke` and `tool.approval`, so a call
  waiting for approval does not hold a slot.
- Other plugins can provide admission too (for example per-session or
  per-tool limits) without changing `ToolInvoker`; mounting one is a trust
  decision, like any `tool.before_invoke` handler.
- Loops run the calls of one turn one after another, so the limit only
  matters when several sessions, scheduled jobs or frontends run turns at
  once.
