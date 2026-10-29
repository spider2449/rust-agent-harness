# ADR 0059: Check Every Plugin Configuration Before The Boot Mounts Anything

Status: Accepted

Date: 2026-10-29

Amends item 6 of [ADR 0044](0044-plugin-configuration-check.md).

## Context

ADR 0044 made each plugin check its configuration (`Plugin::check_config`)
before the kernel mounts it. `rah` boots plugin by plugin, so a check ran
only when its plugin's turn came: a mistake in the tenth plugin stopped the
boot after nine had mounted, and a file with three mistakes took three
starts to fix. With `extends` (ADR 0058) a user writes a few lines and
expects to hear about all of them at once.

After studying DeepSeek Harness, a JSON Schema for plugin configurations was
considered for editor completion. ADR 0044 rejected that for reasons that
still hold (a second form of each type, a validator dependency, checks done
in code it cannot express), so this ADR keeps `check_config` as the only
check and runs it earlier instead.

## Decision

1. **`rah_cli::check_distribution`** runs, for every plugin of a
   distribution and on the configuration `boot` would pass (data paths
   resolved), the check the kernel would run at mount, and returns every
   problem in mount order. It also reports what `boot` refused one at a time:
   an unknown `use` name, a `process` or `wasm` plugin without an `id`, and
   an instance id used twice.
2. **It starts nothing.** A catalog plugin is constructed, which has no side
   effects, and `check_config` is pure (ADR 0044). A process or WASM plugin
   gets only its launch configuration checked, as its host's `check_config`
   does; reading its manifest would start its program or compile its
   component, so that still happens at mount. A panicking check is a
   refusal, as in the kernel.
3. **`boot` runs it first.** When it finds a problem, the boot mounts
   nothing, not even the `distribution` plugin, and fails with
   `BootError::InvalidConfig`, listing every problem. A problem of
   `policy-authority` says "refusing to run unguarded". This replaces
   reporting a refused policy configuration as `PolicyInactive` (ADR 0044,
   item 6); `PolicyInactive` remains for a policy that does not become
   active for another reason.
4. **`rah --check-config`** runs the same check on the distribution `rah`
   would run (the preset or file, `--with`, `--root`, `--model` and the other
   options applied), prints every problem, and exits with 2 when there is
   one, 0 otherwise. It starts no plugin and no program.
5. The kernel still checks at every mount and `update_config`; this check
   is in addition to it, not instead of it.

## Consequences

- A refused distribution leaves nothing mounted, which is stricter than
  before for the policy: no plugin, not even the admin service, runs
  before the configuration is known to be accepted.
- One start or one `--check-config` lists every configuration mistake.
- A configuration that passes can still fail when a plugin starts: a
  missing API key, a directory that must exist, a process plugin's program
  that does not start or its own configuration. `--check-config` says so.
- Only `rah-cli` changes. The kernel, `rah-api`, the plugin protocol and
  the plugins are unchanged.
