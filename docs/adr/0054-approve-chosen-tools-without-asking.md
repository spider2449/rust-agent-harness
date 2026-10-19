# ADR 0054: The Policy Can Allow Chosen Tools Without Asking, and `--approve` Sets It For One Run

Status: Accepted

Date: 2026-10-19

## Context

`rah --prompt` answers a policy `Ask` from standard input: only a line with
`y` or `yes` approves, and end of input denies. That is the right default, but
it makes a scripted run hard. A script has to feed the answers in the order
the model asks, and on Windows PowerShell 5.1 a piped `y` can arrive with a
byte order mark and be read as a refusal (seen on 2026-10-19: every approval
was denied although `y` was piped).

The policy already has `decisions`, which can fix `allow` for a tool. It is
not suited to a command line option, because a decision is looked at before
the permission assignment: `decisions = { "x" = "allow" }` allows a tool that
has no assignment, and one that is misspelled or not mounted does nothing
harmful only by accident. AGENTS.md says a missing assignment fails closed.

ADR 0050 solved the same problem for `shell.exec` commands: a rule that only
turns an Ask into an Allow. This ADR does the same for whole tools.

## Decision

1. **`policy-authority` gets `approve`**, a list of exact tool names. A call
   to a listed tool whose decision is an Ask is allowed without asking.
2. **It only turns an Ask into an Allow.** It does not apply when a
   `decisions` entry covers the tool (the host's explicit choice wins,
   including `ask` and `deny`), when the tool has no assignment (denied), or
   when its level is neither allowed nor asked (denied). A tool that is not
   listed is judged as before.
3. **Exact names only.** A pattern, `*`, or an empty name is refused when the
   policy is configured, so one option cannot approve every tool. A name
   that no mounted tool has approves nothing.
4. **`rah --approve <tool,...>`** (repeatable, comma separated) adds the names
   to `approve` of every mounted `policy-authority` for that run. It is the
   same as writing `approve` in the distribution file; nothing is saved. It
   fails when no `policy-authority` is mounted, because without a policy
   nothing asks and the option would mean nothing. At start, `rah` prints a
   warning that lists the tools that now run without asking.
5. **The model cannot reach it.** The list comes from the command line or
   the host's configuration, like every other policy setting. Tool metadata
   and model output cannot add to it.

## Consequences

- No change to the kernel, the service contracts in `rah-api` or the plugin
  protocol. `tool.before_invoke` and `tool.approval` run as before; the
  option only changes what the policy answers.
- A scripted run can allow exactly the writes it expects, for example
  `--approve repo.create-file,repo.edit-files`, and still be refused
  everything else. A call that arrives with other arguments is allowed too:
  `approve` trusts the tool, not one call. Use `commands` (ADR 0050) when the
  arguments matter.
- `rah-desktop` and the Web UI keep their own interactive approval; the
  option is for the terminal frontend and is not added to `rah-desktop`.
- The warning prints on standard error and does not stop the run, like the
  warning for a run without a policy.
