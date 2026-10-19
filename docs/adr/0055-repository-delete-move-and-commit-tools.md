# ADR 0055: `repo.delete-file`, `repo.move-file` And `repo.commit`

Status: Accepted

Date: 2026-10-19

## Context

ADR 0036, section 4, ported only `repo.patch`, `repo.edit-files` and
`repo.create-file` to `tools-repo-write` and left the other legacy mutation
tools on the legacy line. The pivot plan gave the reason: those tools carried
heavy HostExplicit review flows that had to be simplified first.

Those flows are gone from `master`. Confirmation is the policy's `Ask`
outcome, shown by the frontend, and every call goes through
`ToolInvoker` and `tool.before_invoke`. What is left of the old tools is a
plain capability, and its absence costs something: a model that has to delete
or rename a file can only write a unified diff for `repo.patch`, which is easy
to get wrong, and it cannot commit what it did.

This ADR amends ADR 0036, section 4, on the user's instruction (2026-10-19).
It adds new tools to the plugin. It does not restore any legacy crate or any
HostExplicit flow.

## Decision

1. **`repo.delete-file {path}`** deletes one regular file. **`repo.move-file
   {from, to}`** moves or renames one regular file to a path that does not
   exist, creating missing parent directories. Both are declared `write`.
2. **They act on files, not on links or directories.** A symbolic link, a
   directory or a missing path is refused, so a link inside the root cannot
   make either tool act on what it points to. `move-file` never overwrites.
3. **Paths follow the rules of the other tools**: safe repository-relative
   paths, resolved inside the canonical root, nothing under `.git` or
   `.rah/`. The two tools take the same per-file locks as `repo.edit-files`
   (taken in sorted order), so they do not race an edit.
4. **`repo.commit {message, paths}`** stages the named files (new, changed or
   deleted) and commits only them, so something staged earlier is left out.
   It is declared `execute`: it writes Git history and runs the repository's
   hooks, which are code. It never amends, never skips hooks, never pushes,
   and has no option to do any of those.
5. **The commit identity is host configuration.** `repo.commit` is provided
   only when the plugin's `commit = { name, email }` is set, and it commits
   as that identity. Git's own configuration files are not read (the plugin
   already sets `GIT_CONFIG_GLOBAL` to the null device), so no global setting
   can change what a commit does. Without `commit`, the tool does not exist.
6. **`paths` are literal files.** Git runs with `--literal-pathspecs`, so
   `*` or `?` in a name is not a pattern, and a directory is refused instead
   of committing everything under it. Pathspec magic starting with `:` is
   already refused by the path rules.
7. **Like `repo.patch`, `repo.commit` runs only when Git's top level is the
   configured root**, and it holds every write lock while it stages and
   commits.
8. **The presets assign** `repo.delete-file` and `repo.move-file` `write` and
   `repo.commit` `execute`. An assignment for a tool that is not provided is
   harmless.

## Consequences

- No change to the kernel, to `rah-api` or to the plugin protocol.
- RAH keeps no copy of a deleted file. A file Git tracks can be restored from
  Git; one it does not track cannot. The tool description says so.
- A commit hook can run any program the repository defines. The path tools
  (create, edit, delete, move) refuse any path under `.git`, so they cannot
  plant a hook; a user who mounts `tools-shell` can still write one.
  `execute` is the level that matches this.
- `format-on-write` and `lsp-diagnostics` look at `write` tools only and read
  `path`, `edits[].path` or `patch`, so they ignore the move tool's `from` and
  `to`, the deletion (the file is gone) and the commit.
- Still on the legacy line: stage and unstage, branch creation, directory
  creation and the rest. A user who needs them has `tools-shell`.
