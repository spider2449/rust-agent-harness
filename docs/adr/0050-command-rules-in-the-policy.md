# ADR 0050: The Policy Can Allow Chosen Commands Without Asking

Status: Accepted

Date: 2026-10-17

## Context

`shell.exec` declares `execute`, which the presets' policy asks about on every
call. Asking before `git status` and before `git push` alike teaches people to
approve without reading, which weakens the one confirmation that matters.

opencode answers with a shell parser. It splits a command string into
commands, matches each against a prefix list, and treats what it cannot
analyze (command substitution, heredocs, dynamic command names) as *opaque*,
which is only a parsing limit and never an allow.

RAH has no command string. `shell.exec` takes a `program` and an `args`
array and starts the program without a shell (AGENTS.md: a fixed program plus
an argument vector, never shell interpolation). The call is already split, and
nothing can be opaque, so a parser would add attack surface and no accuracy.
What remains useful is the matching.

## Decision

1. **`policy-authority` gets `commands`**, a list of rules for the tool
   `shell.exec`. A rule is `["program", "arg", ...]`, matched against the
   call's `program` and `args` as exact strings. A last element `"*"` also
   accepts any further arguments, including none.
2. **A rule only turns an Ask into an Allow.** It does not apply when a
   `decisions` entry covers `shell.exec` (the host's explicit choice wins,
   including `deny`), when the tool has no assignment, or when its level is
   denied. A call that matches no rule is judged as before.
3. **The input is untrusted.** `program` must be a string and `args`, when
   present, an array of strings; anything else matches no rule and is asked
   about. `./git`, `/usr/bin/git` and `Git` are different programs from
   `git`. The rule trusts the sandbox's resolution of the bare name, as the
   tool already does.
4. **Exact is the default.** `["git", "status"]` allows `git status` and not
   `git status -s`. The wildcard is explicit and the documentation says what
   it costs: `["git", "diff", "*"]` also allows `git diff --output=file`,
   which writes a file, and some programs run other programs through their
   options. Write a wildcard rule only for a program whose every option is
   acceptable.
5. **The rules apply to the repaired input** (ADR 0049), because the policy
   sees what the tool receives.

## Consequences

- The policy knows one tool name and its two argument names. That is a
  convention of `tools-shell`, stated here and tested; a different shell tool
  is not covered until it is added on purpose.
- No parser, no WASM, no new dependency.
- A user who wants more than one tool covered, or deny rules by prefix, has
  `decisions` for whole tools and, later, a further ADR for rules on other
  tools.
