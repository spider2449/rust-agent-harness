# ADR 0053: The Project's Formatter Runs After Write Tools

Status: Accepted

Date: 2026-10-17

## Context

Models write code in a style that is close to, but not, the project's. The
mismatch shows up later as a failed `cargo fmt --check` or a noisy diff.
opencode runs a formatter after `edit` and `write`, choosing it from the file
extension and from the project's configuration files.

This is the same shape as ADR 0048: react to a completed write, by file, and
tell the model something about it. It needs no contract of its own.

## Decision

1. **A hook plugin, `format-on-write`**, on `tool.after_invoke` at priority
   -5, so that it runs before `lsp-diagnostics` (-10) and the diagnostics
   describe the formatted file. When a tool declared `write` completed
   without error, it finds the files from the call's arguments (the same
   convention as ADR 0048), runs the formatter for each file's extension
   through the `sandbox` service (`ExecEnv::run`: a fixed program, an argument
   vector with `$FILE` replaced by the root-relative path, a timeout, bounded
   output) and appends one note listing the files the formatter changed. The
   note says that the file on disk now differs from what the model wrote and
   must be read again before it is edited, because `repo.edit-files` matches
   exact text. Nothing is appended when no file changed.
2. **Defaults run only code that cannot run project code.** `rustfmt` and
   `gofmt` always; `ruff` when `ruff.toml`, `.ruff.toml` or a `[tool.ruff`
   section of `pyproject.toml` is found from the file's directory up to the
   root; `black` for `[tool.black`; `clang-format` for a `.clang-format` file.
   `prettier` is not a default: its configuration can be JavaScript and its
   plugins are code, so a file the model writes could make the formatter run
   it. A user who accepts that names it in `formatters`.
3. **Failures are quiet for defaults and loud for named formatters.** A
   default that is not installed is skipped for five minutes without a word;
   a formatter the user wrote that fails, is missing or times out is reported
   in the note, in one bounded line.
4. **Configuration** follows ADR 0048's shape (`formatters` by name with
   `command`, `extensions`, `env`, `disabled`, plus `requires` for the files
   that make it apply; `defaults`, `timeout_ms`, `max_files`).

## Consequences

- A formatter changes files without a second approval. It changes only files
  the approved write already touched, and only by the project's own
  formatting rules, but it is a second writer: the session log's tool result
  shows the model's write, and the note explains the difference.
- A formatter is a program the plugin starts after a `write` approval, so the
  isolation is the sandbox's. Formatters that read project configuration are
  limited to ones that cannot execute it (point 2).
- The text of the file the model holds is stale after a formatting run. The
  note tells it; the next edit fails with "old was not found" if it ignores
  the note, and does no harm.
