# ADR 0048: Language Server Diagnostics Ride the Tool Output Hook

Status: Accepted

Date: 2026-10-17

## Context

A model that edits code cannot see the compiler. It finds out that an edit
broke the build only when it runs a build, if it thinks to. opencode plans
to answer this with a language server runtime that is told about every
written file and whose diagnostics are attached to the write tool's result
(in its v2 sources this is still a TODO beside `edit` and `write`). The
useful part for an agent is that feedback, not code intelligence for a
human editor.

The question for RAH is where it belongs. Options:

1. a new `lsp` service in `rah-api` that write tools call;
2. a hook that watches write tools from outside.

## Decision

1. **A hook, not a service.** The new plugin `lsp-diagnostics` attaches to
   `tool.after_invoke`. When a tool declared `write` completed without an
   error, it reads the files the call named from the call's arguments,
   sends their new text to the language server for their extension and
   appends the diagnostics at or above `severity` to the tool output. It
   changes nothing else: not the result, not the effect, not the policy
   decision. `rah-api`, `rah-protocol` and the kernel do not change.
2. **Paths come from the arguments by convention:** `path`, `edits[].path`
   and the `+++` lines of a `patch`. That is what `repo.create-file`,
   `repo.edit-files` and `repo.patch` take, and what a process plugin that
   writes files can copy. A tool with other argument names is not checked;
   adding a mapping is a configuration change to this plugin, not to the tool.
3. **Servers start through `sandbox`** (`ExecEnv::spawn`), lazily, one
   process per configured server for the life of the plugin's scope. The
   plugin declares `requires = ["sandbox"]` and no other service. A server
   that cannot start or ends is left alone for five minutes.
4. **Configuration** follows opencode's shape (`command`, `extensions`,
   `env`, `initialization`, `disabled`, by server name) and adds defaults for
   `rust-analyzer`, `typescript-language-server`, `pyright-langserver`,
   `gopls` and `clangd`, tried only when the program exists. A failure to
   start a default is silent; a failure to start a server the user wrote is
   reported in the tool output.
5. **Protocol use is minimal:** `initialize`, `initialized`, `didOpen`,
   `didChange` (whole text), `didSave` and `publishDiagnostics`. A
   publication for an older document version is ignored, and publications
   that follow each other within 250 ms count as one.

## Consequences

- No new contract, so nothing else can depend on LSP being present; the
  write tools do not know the plugin exists.
- Diagnostics are an external program's text about untrusted files. They are
  limited in number and length, flattened to one line and introduced as
  output of an external program, not instructions. They grant nothing: a
  tool call that follows still goes through the policy.
- A slow server (first indexing) may not report in `wait_ms`. The output then
  says so, and later checks use what the server has loaded.
- A language server can run code from the workspace (`rust-analyzer` runs build
  scripts and procedural macros), so a file the model writes can run code when
  the server checks it. That turns a `write` approval into code execution at the
  sandbox's isolation; SECURITY.md says to confine it with `sandbox-os`.
- The server runs with the sandbox's isolation, no more. Under `sandbox-os` a
  server that needs the network or a path outside `readable`/`writable`
  fails, and says so.
- Only RAH's own writes are announced. Edits made outside RAH are not sent to
  the server; the next RAH write sends the full text of the file again.
- Code actions, hover, definitions and references are not offered to the
  model. They would be tools and a new service, and are a later decision.
