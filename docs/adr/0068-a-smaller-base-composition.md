# ADR 0068: A Smaller Base Composition

Status: Accepted

Date: 2026-11-06

## Context

The guarded base composition (what a `rah.toml` with user keys starts from,
ADR 0067, and what the built-in presets share) mounted 18 plugins. Five of
them are extras a user may not want and did not ask for:

- `session-titles` makes one extra model request per session;
- `skills-fs` adds a skills list to every model request;
- `scheduler-basic` runs background jobs and subagents (`agent.delegate`);
- `telemetry-log` writes records of every turn to the data directory;
- `plugin-builder` offers the model tools to write plugins.

Each makes `rah config show --plugins`, the Plugins panel and the tool list
longer, and each was added to the presets by an earlier decision (ADR 0062
for telemetry, among others) while features (`--with`, `features = [...]`)
did not yet exist or did not cover them.

## Decision

1. The base composition is 13 plugins: `policy-authority`, `storage-fs`,
   `session-jsonl`, `tools-builtin`, `sandbox-process`, `tools-shell`,
   `tools-repo`, `tools-repo-write`, `context-default`, `tool-input-repair`,
   the model, `loop-react` and `frontend-cli`.
2. The five extras are features: `titles`, `skills`, `scheduler`,
   `telemetry` and `plugin-builder`. A feature brings its policy
   assignments (`skills.*` and `agent.*` at `read`, `plugins.*` as before),
   so the base policy no longer assigns tools that are not mounted. The
   scheduler is placed after the loop it uses.
3. Removing such a plugin in the Web UI's Plugins panel removes its feature
   from `rah.toml`, so it does not come back at the next start.

## Consequences

- A fresh setup is smaller: no extra title requests, no telemetry files, no
  skills list in requests and no plugin tools unless turned on.
- The Web UI's Usage view, generated session titles, background jobs and
  "My plugins" need their feature. Without `titles`, the session list shows
  the start of the first message.
- A file with `extends = "<preset>"` that changes or removes one of these
  plugins now fails to parse (the preset no longer has it); turning the
  feature on is the replacement. `CHANGELOG.md` says so.
- Amends ADR 0062 (every preset mounts `telemetry-log`) and the preset
  contents described in ADRs 0041, 0049 and 0064 as far as these five
  plugins go.
