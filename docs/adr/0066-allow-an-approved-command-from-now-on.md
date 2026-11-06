# ADR 0066: The Web UI Can Allow An Approved Command From Now On

Status: Accepted

Date: 2026-11-06

## Context

Since ADR 0064 the guarded presets mount `shell.exec`, and ADR 0065 tells the
model to investigate with it. Every program run is a policy `Ask`, and ADR
0056 deliberately never offers "Allow for this session" for a tool that needs
`execute`, because that would trust every program with every argument. Users
answered the same `netstat -ano` or `git status` again and again, which is
the approval fatigue ADR 0050 and ADR 0056 warn about.

ADR 0050 already has the safe unit: a `commands` rule matches one program and
its arguments exactly and turns only an `Ask` into an `Allow`. What was
missing was a way to add one without editing the policy by hand.

## Decision

1. **A fourth answer in the Web UI**: "Always allow this command", offered on
   a `shell.exec` approval. It approves the call and adds the call's exact
   `[program, args...]` to the policy's `commands`.
2. **The rule comes from the call, not from the browser.** The server keeps
   the rule it derived when the request was asked; the browser only sends
   `command: true` with its answer (`{type: "approval", id, approve, always,
   command}`; `command` defaults to false). The `approval_required` notice
   carries `command`, the rule, or `null` when none is offered.
3. **Exact only.** No rule is offered when the input is not a program string
   with an array of string arguments, when the program is blank, or when any
   element is `*`, which a rule would read as a wildcard. Other arguments
   are asked about as before.
4. **Stored like a saved Tools form.** The frontend applies the new policy
   configuration through the `admin` service and stores it under
   `settings-tools`, so it survives a restart and shows in **Settings →
   Tools**, where it can be removed. A rule that is already there changes
   nothing. A failure to apply or store is reported to the browser; the call
   that was approved still runs once.
5. **The policy's rules are unchanged** (ADR 0050): a `decisions` entry for
   `shell.exec`, a missing assignment or a denied level still wins over a
   `commands` rule.

## Consequences

- No change to the kernel, `rah-api`, `rah-protocol`, the plugin protocol or
  `policy-authority`. The change is in `frontend-web` only.
- A remembered rule applies in every session and after restarts, unlike ADR
  0056's session memory. It is the user's own answer at the prompt, written
  to the same configuration the user can already edit in the Tools form.
- An exact rule still trusts what that command does. `curl -o file URL`
  writes a file each time it is allowed; the button shows the full command.
- The terminal frontend does not offer it yet.
- The policy plugin restarts to take the new rule; a tool call being judged
  at that moment is judged by the old or the new configuration.
