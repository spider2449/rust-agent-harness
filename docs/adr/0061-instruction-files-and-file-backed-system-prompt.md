# ADR 0061: Instruction Files And A File-Backed System Prompt

Status: Accepted

Date: 2026-10-29

## Context

Item 3 of the harness2 lessons plan (2026-10-29). harness2 (Google
Research) optimizes an agent's setup by editing a directory of files: system
prompt, standing rules ("guardrails"), memory, notes on how to use the tools,
subagents and skills. Two of those have no place in RAH:

- **The workspace's instruction file.** OpenCode, Codex and Claude Code read
  `AGENTS.md` (or a similar name) from the project, so a repository can tell
  any agent how to build, test and behave in it. RAH reads no such file; the
  user has to repeat it in the system prompt or in every turn.
- **A setup kept as files.** `context-default` takes its system prompt only
  as a string in the plugin configuration, so the prompt cannot be a file
  under version control next to the rest of a setup, and standing rules or
  tool notes have nowhere to go but that one string.

Constraints: the kernel and the service contracts stay as they are
("Replace, don't patch": a new behavior is a new plugin or hook), and text
from a workspace is not trusted.

## Decision

1. **A new plugin, `context-instructions`**, on the existing
   `model.before_request` waterfall. On every request it reads the first of
   `names` (default `["AGENTS.md"]`) that exists directly in the `sandbox`
   service's workspace root, and every file in `files`. It adds them as one
   system item after the leading system items, each under a heading naming
   its file. Each file is cut at `max_bytes` (32768), with a note.
2. **Only the workspace root.** Parent directories are not searched, so the
   plugin never reads a file the user did not expect (such as one in a
   directory above a cloned repository). `names` takes plain file names;
   paths are refused by the configuration check.
3. **Read on every request**, as `skills-fs` and memory notes are: edits
   apply at once, and switching the workspace switches its file. A missing
   workspace file is normal and silent. A file the user listed in `files`
   that is missing or unreadable is reported once with a `HostNotice` (ADR
   0057), never to the model.
4. **`files` covers standing rules and tool notes.** RAH adds no separate
   mechanism to override tool descriptions; text about how to use the tools
   is one more instruction file. `rah` resolves relative `files` paths
   against the data directory, like other data paths (a list of paths is
   resolved element by element).
5. **`context-default` gets `system_file`**, exclusive with `system`: the
   system prompt is read from that file when the plugin starts, and a file
   that cannot be read fails the mount with the path and the reason. It is
   read once, not per request, because the system prompt is configuration;
   restarting the plugin reads it again.
6. **Opt-in.** The plugin is mounted by the feature `instructions`
   (`--with instructions`, or `features = ["instructions"]`), not by the
   presets. A repository's instruction file changes what the model is told,
   and the user should choose that.
7. **Text only.** The files grant no permission and define no tools. Every
   tool call still goes through `ToolInvoker`, the `tool.before_invoke`
   chain and the policy.

## Consequences

- No change to the kernel, `rah-api` contracts, `rah-protocol` or the plugin
  protocol. One new plugin crate depending on `rah-api` and `rah-protocol`.
- A workspace file is text written by whoever controls the repository. It can
  try to steer the model (prompt injection) like any file the model reads; it
  cannot change what the policy allows. `docs/SECURITY.md` says so.
- The instruction file costs context on every request, bounded by
  `max_bytes` per file.
- The workspace file needs a mounted `sandbox` service for its root; without
  one only `files` are read.
- A multi-folder workspace reads the instruction file of the sandbox root
  only.
