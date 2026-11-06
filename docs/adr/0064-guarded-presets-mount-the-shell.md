# ADR 0064: Guarded Presets Mount The Shell

Status: Accepted

Date: 2026-11-06

## Context

No preset mounted `tools-shell`; program execution needed `--with shell` or
an edited `rah.toml`. In practice an agent without it cannot do much of the
ordinary work it is asked for. A request such as "look at localhost:7820"
needs a program (`netstat`, `Get-NetTCPConnection`, `curl`) to find what
listens on the port. With only the file and repository tools, a local model
searched files, found nothing and ended without an answer, while other
harnesses that give the same model a shell finished the task. Users do not
know that the capability is missing, so they do not know to enable it.

ADR 0036 and `docs/SECURITY.md` call the shell an opt-in plugin. The point of
that rule is that execution is a plugin the configuration mounts, never part
of the kernel, and that it runs only through a `sandbox` and the policy. It
does not require the built-in presets to leave it out.

## Decision

1. **Every guarded preset mounts `tools-shell`** (`default`, `dev`,
   `openai`, `anthropic`, `deepseek`), right after `sandbox-process`, with no
   `allow_programs` list.
2. **The presets' policy assigns `shell.exec = "execute"`**, which the
   presets ask about. Every program run is confirmed by the user, as for
   `repo.commit`. ADR 0050 command rules can stop the asking for exact
   commands.
3. **`minimal` does not mount it.** It has no policy, so nothing would ask
   before a program runs.
4. **`--with shell=<programs>` still works** on a preset: it sets
   `allow_programs` on the mounted plugin instead of mounting a new one.
5. Nothing else changes: `shell.exec` still takes a fixed program and an
   argument vector, runs through the `sandbox` service with a cleared
   environment, bounded output, timeout and cancellation, and an interrupted
   program is recorded as `uncertain`.

## Consequences

- No change to the kernel, `rah-api` contracts, `rah-protocol` or the plugin
  protocol; only the preset composition in `rah-cli`.
- The model sees `shell.exec` by default and will try to use it, so users of
  the guarded presets are asked more often. Declining an approval is a normal
  tool error the model can work around.
- `sandbox-process` is process supervision, not an OS sandbox. An approved
  program runs with the user's rights inside the workspace root as its
  working directory; `docs/SECURITY.md` keeps saying so.
- A user file with `extends` and `remove = ["tools-shell"]` turns it off.
