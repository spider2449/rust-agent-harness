# ADR 0056: A Frontend Can Allow A Tool For The Rest Of The Session

Status: Accepted

Date: 2026-10-19

## Context

A policy `Ask` is answered once per call. A model that writes five files asks
five times, and a user who answers the same question five times starts to
answer without reading, which weakens the confirmation that matters (the
reasoning of ADR 0050). Neither frontend had a way to say "this tool is fine
for this session".

ADR 0050 (`commands`) and ADR 0054 (`approve`) let the *host's configuration*
allow chosen calls or tools. This ADR is the interactive counterpart: the
user's own answer, given at the prompt, for one session.

## Decision

1. **Both frontends can offer a third answer** next to Allow and Deny. In the
   terminal it is `a` (or `always`), shown as `[y/N/a=always]`. In the Web UI it
   is the button "Allow for this session". It approves the call and allows the
   same tool for the rest of the session.
2. **The unit is the tool name within one session.** `(session, tool)`. It is
   not per argument, so it trusts the tool for that session: every argument
   the model sends to it is allowed. Another tool, another session, and a
   session opened again later are asked as before.
3. **The frontend keeps it, in memory, for the life of the process.** It is not
   written to the session log, to `storage` or to any configuration, and it
   cannot be set from outside. Ending the process, or `/new` in the terminal,
   ends it.
4. **A tool that needs `execute` is never offered it.** That covers a tool
   that declares `execute` (`shell.exec`) and a tool the policy treats as
   `execute` through its assignment: the approval request carries only the
   declared permission, so the frontend also reads the policy's reason text
   (`requires \`execute\` permission`) and refuses when it names `execute`. The
   check can only make the rule stricter. The Web server enforces it itself
   and ignores a browser that sends `always` for such a tool; the button
   follows `may_remember` in the notice and is only a convenience.
5. **Nothing else changes.** The frontend answers the same `tool.approval`
   hook with a boolean, so `rah-api` and the plugin protocol are unchanged.
   The policy, `tool.before_invoke`, admission and the `ToolInvoker` run as
   before: a call the policy denies never reaches the approver, and a
   remembered tool is only an Ask answered in advance.
6. **The Web message gains an optional field.** `{type: "approval", id,
   approve, always}`; `always` defaults to false, so an older page keeps
   working, and the `approval_required` notice gains `may_remember`.

## Consequences

- A `decisions` entry of `ask` still asks the first time. After the user
  answers `always`, that frontend stops asking for that tool in that session:
  the answer is the user's own, given at the prompt, not metadata. A host that
  wants a tool to ask every time cannot rely on `ask` alone against a user who
  chooses `always`; `deny` or removing the assignment turns the tool off.
- There is no list or revoke command yet. The way back is `/new`, another
  session, or restarting. A later change can add one.
- Process plugins that answer `tool.approval` are not affected; each approver
  decides for itself.
- The check on the reason text is a heuristic that exists because the request
  has no effective level. Adding one to `ApprovalRequest` would let both
  frontends drop it, and needs its own ADR because the hook payload reaches
  process plugins.
