# ADR 0060: Undo The File Changes Of A Session

Status: Accepted

Date: 2026-10-29

## Context

Item 7 of the functional gaps plan (2026-10-19): after a write tool changed a
file there is no way back inside RAH. The user has to know Git, and a file
Git does not track, or a change on top of uncommitted work, cannot be
recovered at all. `repo.delete-file` says so in its description ("RAH keeps
no copy").

AGENTS.md rules out anything that suggests external effects can be rolled
back. An undo is acceptable only for what RAH can prove: files RAH itself
wrote, whose content is still exactly what RAH left there.

Where the record can be kept:

- A separate plugin on `tool.before_invoke` cannot take the copy. That chain
  is the policy's: a handler there counts as a guard (`is_guarded`), and once
  the policy answers `Allow` later handlers do not run.
- `tool.after_invoke` comes too late: the file is already changed.
- `tools-repo-write` is the only code that knows, under its per-file locks,
  the content before and after each write. It keeps the record.

## Decision

1. **`tools-repo-write` keeps an undo history per session, in memory.** For
   every write tool call that completes (`repo.create-file`,
   `repo.edit-files`, `repo.delete-file`, `repo.move-file`, `repo.patch`), it
   records each file's content before and after the call, read under the
   same locks as the write. A missing file is recorded as missing. A call
   that touches a file it cannot copy (larger than `max_file_bytes`, not a
   regular file) is not recorded. `repo.commit` changes Git history, not the
   files, and is not recorded.
2. **The record follows the file to the end of the call.** A
   `tool.after_invoke` handler of the same plugin runs after every other
   (priority -100) and reads the files again, so a formatter that rewrote
   the file (`format-on-write`) is part of the change. To find the record of
   a call, `ToolContext` gains `call: ToolCallId`, the id the
   `ToolInvoker` already allocates.
3. **Undo takes back the newest change of a session, all or nothing.** Every
   file must still match what the change left (content, or still missing).
   Then each file gets its earlier content back, or is deleted when the
   change created it. If any file differs, nothing is written and the change
   is dropped from the history with the names of the files that differ.
   Older changes can still be undone, and the same check protects them.
4. **Only the user undoes, through a frontend.** The new `file-undo`
   service (`rah_api::undo::FileUndo`: `changes`, `undo_last`, `take_notes`)
   is called by `frontend-cli` (`/changes`, `/undo`) and `frontend-web`
   (`GET /api/sessions/{id}/changes`, `POST /api/sessions/{id}/undo` and a
   button). It is not a tool, so the model cannot call it. Undo is the
   user's own action on their workspace, like an editor's, so it does not go
   through `tool.before_invoke`.
5. **The model is told at its next turn.** An undo leaves a note with the
   provider. A frontend that starts a turn takes the session's notes
   (`rah_api::undo::with_notes`) and puts them before the user's text, so
   the note is in the session log like any input and replay shows the same
   context.
6. **The history is bounded and forgotten.** At most 50 changes per session
   and 64 MiB in total per plugin instance, oldest first. It lives as long as
   the plugin instance. A restart, a configuration change or switching the
   workspace folder (which reconfigures `tools-repo-write`) forgets it.
   Nothing is written to disk.

## Consequences

- `rah-api` gains the `undo` module and `ToolContext.call`. Code that builds
  a `ToolContext` itself (tests) must set it. The kernel and the plugin
  protocol do not change; a process plugin cannot provide `file-undo` yet.
- Undo covers the repository write tools only. `shell.exec`, MCP tools and a
  process plugin's tools can change files RAH has no copy of. An undo of an
  older change can therefore meet a file changed by something else, and
  refuses it.
- An undo does not touch Git: a change that was committed since stays in the
  history; the undo changes the work tree only.
- With several `tools-repo-write` instances, a frontend undoes the newest
  change of the session among them.
- The undo history holds file contents in memory. It is bounded (decision 6),
  never leaves the process and is not shown to the model.
