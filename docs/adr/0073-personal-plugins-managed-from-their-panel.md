# ADR 0073: Personal Plugins Are Managed From Their Panel

Status: Accepted

Date: 2026-11-10

## Context

`plugin-builder` saved a personal plugin and stopped there. To run it, the
user copied a `[[plugin]]` entry and the policy assignments from `mount.toml`
into `rah.toml`, ran `rah config check` and restarted. A missed assignment
left every tool of the plugin denied. A changed plugin ran only after another
restart, and nothing showed that the running one was the earlier save. A
deleted plugin left its entry in `rah.toml`, which then named files that no
longer existed. A copy to keep needed a file path typed into a form.

The rule behind the manual steps is sound: saving is something the model
does, mounting is the user's trust decision, and a plugin must not gain a
permission by being written. The manual steps are not the only way to keep
it. A view action runs only on the user's click in the Web UI
(ADR 0062); the model cannot reach it.

On Windows a process keeps its working directory open, so a running plugin
whose working directory is its own directory cannot be replaced or deleted.

## Decision

1. **The "My plugins" view is where a personal plugin is managed.** Each
   saved plugin has a state there: not enabled, enabled, or enabled with a
   later save waiting. The built-in Plugins panel keeps listing the mounted
   ones as before.
2. **Enable, reload and disable are view actions of `plugin-builder`**,
   carried out through the `admin` service the distribution already provides
   (`mount`, `unmount`, `configure`, `persist`). `rah-api`, the kernel and
   the plugin protocol are unchanged. Without an `admin` service the actions
   are refused and `mount.toml` remains the way.
   - *Enable* shows the plugin's tools with the permissions it declares, its
     services and hooks, and needs a ticked confirmation. It then adds the
     tool assignments the policy lacks to every running policy, mounts the
     plugin as a `process` plugin under its name, and asks the distribution
     to keep both in its configuration file. A plugin that does not become
     active is unmounted again and the assignments added for it are taken
     back.
   - *Reload* runs the last save of an enabled plugin: unmount, then the
     steps of enable, with the same confirmation. A plugin that does not
     start again is left disabled and removed from the file, so the next
     start of `rah` does not fail on it.
   - *Disable* unmounts the plugin, removes its entry from the file and
     removes the assignments of its tools that still hold the declared
     level. The files stay.
   - *Delete* disables the plugin first.
3. **The policy stays the authority.** An assignment the policy already has
   is never changed. The effective permission is the stricter of the
   assignment and the tool's declaration (ADR 0036), so an assignment left
   from an earlier save cannot loosen a tool that now declares more.
4. **The model gets no new tool.** `plugins.save` still never mounts, runs
   or assigns anything. Saving an enabled plugin leaves the earlier one
   running until the user reloads it.
5. **A personal plugin runs in the plugin directory, not in its own.** The
   mount entry, in `mount.toml` too, sets `cwd` to the directory that holds
   the plugins, so a running plugin can be replaced and deleted on Windows.
   A plugin is recognized as the builder's by its entry file's directory.
6. **The actions run as tasks of the builder's scope** and finish when the
   page stops waiting for them, so a slow start does not leave a plugin
   mounted but not kept.
7. **Backups have a fixed place.** *Back up* writes every saved plugin to a
   new bundle in `backups/` under the plugin directory and notes which were
   enabled; *restore* picks one from a list. A delete writes a backup first.
   Restoring, like importing, puts each plugin through the checks of
   `plugins.save` and enables nothing. Export and import to a chosen path
   stay, for another machine.

## Consequences

- A personal plugin is written, enabled, changed, removed and backed up
  without editing `rah.toml` or restarting.
- The earlier statement that `plugin-builder` never mounts a plugin and never
  touches the policy or `rah.toml` now holds for the model's tools only. The
  trust decision moved from editing a file to a confirmed click by the local,
  token-holding user of the Web UI, as for every other `admin` operation.
- `plugin-builder` names two plugin kinds of the distribution (`process` and
  `policy-authority`), as `frontend-web` does for its switches. A
  distribution with another policy plugin gets the mount without the
  assignments, and the message says so.
- The terminal REPL has no views. Without the Web UI, `mount.toml` remains
  the way to mount a personal plugin.
- Whether a later save is waiting is kept in memory. After a restart of
  `rah` the running plugin is the last save, so nothing is lost.
