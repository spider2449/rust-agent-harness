# ADR 0074: The Approval Request Carries The Effective Level

Status: Accepted

Date: 2026-11-10

## Context

A frontend may keep the answer "allow this tool for the rest of the session"
(ADR 0056), but never for a tool that needs `execute`. The approval request
carried only the permission the tool *declares*. The level a call is judged
at is the stricter of that and the host's assignment, and only the policy
knows it. ADR 0056 therefore had both frontends read the policy's reason
text and refuse when it named `` `execute` ``.

That check depended on wording. A tool assigned `execute` and also listed
under `decisions = ask` gets the reason "the policy asks before tool ... runs",
which names no level, so the tool could be allowed for the session. Another
policy plugin words its reasons its own way. ADR 0056 named the fix and left
it to an ADR, because it changes a hook payload.

## Decision

1. **`ToolDecision::Ask` gains `level: Option<PermissionLevel>`**: the level
   the policy judged the call at. `None` means the policy gave none.
2. **`ApprovalRequest` gains the same `level`.** The `ToolInvoker` copies it
   from the decision unchanged. It comes from the mounted policy, which is
   trusted code; nothing from the model or the tool reaches it.
3. **`ApprovalRequest::may_remember` is the one rule** both frontends use:
   the answer may be kept only when the level is known and is not `execute`,
   and the tool does not declare `execute`. The reason text is only shown.
4. **An unknown level is not remembered.** A request without a level asks
   every time. This fails closed: an approver cannot tell a harmless call
   from an `execute` one without it.
5. **`policy-authority` reports the stricter of assignment and declaration**
   for both kinds of Ask (by level and by a `decisions` entry). A `decisions`
   entry of `ask` for a tool without an assignment has no level: tool
   metadata alone sets none.
6. **A process plugin's `tool.before_invoke` reply may carry it**:
   `{"decision": "ask", "reason", "level"}`, with `level` one of `none`,
   `read`, `write`, `execute`. It is optional, so existing plugins keep
   working; their questions are asked every time. An unknown value is a
   malformed decision, which denies the call.
7. **The Web notice `approval_required` gains `level`** next to the declared
   `permission`. `may_remember` is still computed by the server.

## Consequences

- A tool the host assigned `execute` is asked about every time, whatever
  made the policy ask and however the reason is worded.
- A tool listed under `decisions = ask` without a `tools` assignment was
  offered "allow for this session" when it did not declare `execute`. It is
  now asked every time. Assigning it a level in `tools` brings the offer back.
- A policy plugin written against the earlier `ToolDecision` must set the
  field; in this repository that is `policy-authority` and the process host.
- This amends point 4 and the last consequence of ADR 0056; the rest of it
  stands. `tool.approval` is still not offered to process plugins.
