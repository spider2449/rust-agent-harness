# RAH As A harness2 Executor

[harness2](https://github.com/google-research/harness2) (Google Research,
arXiv 2610.14224) improves an agent's setup without changing the model: it
runs a task, proposes edits to the files around the model, runs the task
again with each candidate, and composes a better setup from the differences
between the runs. It ships executors for OpenCode and Codex. This page says
what an executor for RAH needs and which RAH options provide it. The adapter
itself is Python and belongs on the harness2 side; nothing here is part of
the RAH workspace.

## The editable setup

harness2 edits eight components. In RAH each is a file or a plugin
configuration, so one directory can hold a whole setup:

| harness2 component | RAH |
| --- | --- |
| system prompt | `context-default` `system_file` (ADR 0061) |
| guardrails (standing rules) | a file in `context-instructions` `files` (ADR 0061) |
| memory | `context-default` `memory_file` or `memory_dir` |
| tool guidance | a file in `context-instructions` `files` |
| subagents | `scheduler-basic` `agents_dir`: one `<name>.md` per subagent |
| skills | `skills-fs` `dir`: one `<name>/SKILL.md` per skill |
| scripts | none: skills are text; programs run through `tools-shell` and the policy |
| plugins | the distribution file (`rah.toml`), which can `extends` a preset (ADR 0058) |

A setup directory might look like this, with `rah.toml` naming the other
files by relative paths. `rah` resolves relative data paths against the data
directory, so a run copies the setup into a fresh directory and uses that as
its `--data-dir`:

```text
setup/
  rah.toml            extends = "openai"; features = ["instructions"]
  system.md           context-default system_file
  rules.md            context-instructions files
  tools.md            context-instructions files
  memory.md           context-default memory_file
  skills/<name>/SKILL.md
  agents/<name>.md
```

The workspace's own `AGENTS.md` is read too when `instructions` is on. A
setup under evaluation should not rely on it unless every task has one.

## One run

Each candidate run gets its own copy of the setup as its data directory, so
sessions, memory and storage of runs never mix:

```text
rah --config runs/<run-id>/rah.toml --data-dir runs/<run-id>
    --root <task workspace> --prompt "<task>"
    --summary runs/<run-id>/summary.json --approve <tool>,<tool>
```

- `--check-config` with the same options before any run reports every
  configuration problem and starts nothing (ADR 0059). It does not check API
  keys or programs, which are found only at start.
- `--summary` writes how the run ended (`outcome`, `error`), `wall_ms`, model
  requests and retries, summed token `usage` (counts the provider did not
  report are `null`) and the tool calls with how many failed, are
  `uncertain` or have no recorded result. It never contains a price; convert
  tokens with the provider's prices.
- The session id is on the first line of standard output (`session <id>`).
- `--approve` lets the named tools run without a person answering. Any
  other tool the policy asks about is answered from standard input, and
  denied when there is none.

## Reading the run back

```text
rah --data-dir runs/<run-id> --session <id> --export trace
```

prints the session in the line grammar harness2 reads: `[USER]`,
`[ASSISTANT]`, `[ACTION] tool(args)`, `[OBSERVATION]`,
`[OBSERVATION ERROR]` and `[ERROR]`, with arguments cut at 400 characters
and other text at 50 000, each cut followed by the number of characters left
out. A result whose effect is uncertain says so, and a call with no recorded
result shows as an `[OBSERVATION ERROR]`. `--export json` gives every record
for an adapter that renders its own format.

Deliverables are whatever the task wrote in the workspace (`--root`).

## Safety

harness2 runs its executors with unrestricted network access, and its Codex
executor with the sandbox turned off. For RAH:

- Keep the policy (`policy-authority`, in every preset except `minimal`) and
  approve only the tools a task needs.
- `sandbox-process` supervises programs; it is not an OS sandbox. Use
  `sandbox-os` for file confinement (and, on Linux, a network filter). RAH
  does not isolate the network otherwise.
- A setup written by an optimizer is untrusted text for the model. It grants
  nothing: the policy decides every tool call.
- Never replay a run's tool calls: a timed out or cancelled call may have
  happened. Compare runs from their traces.
