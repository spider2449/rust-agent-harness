# ADR 0044: Plugins Check a Configuration Before It Is Applied

Status: Accepted

Date: 2026-10-15

## Context

`PluginManifest` had an optional `config_schema` field (pivot plan, section
3). Nothing set it and nothing validated it: a plugin found out that its
configuration was wrong only in `apply`, after the kernel had already
stopped the running instance and its dependents.

That matters most for configuration changes at runtime. The Web UI's
Settings and Plugins panel call `admin.configure`, which calls
`Kernel::update_config`. With a typo, the kernel stopped the plugin, `apply`
failed and the plugin became `Failed`. For `policy-authority` that meant no
`tool.before_invoke` handler was mounted until someone noticed, so tool calls
ran unguarded in between; the Web UI then restored the old configuration on
its own.

Every static plugin already deserializes its configuration into a typed
struct with `deny_unknown_fields`, and most check a few values after that
(for example `keep_chars < trigger_chars`, a loopback endpoint, a level not
listed in both `allow` and `ask`). A JSON Schema would restate these types in
a second form, needs a validator dependency in the kernel or `rah-api`, and
still could not express the checks done in code.

## Decision

1. **`Plugin::check_config(&self, config) -> Result<(), PluginError>`.** A
   provided method on the kernel's `Plugin` trait; the default accepts every
   configuration. It must not have side effects (no file system, network or
   process access); it is a pure check of the value.
2. **The kernel calls it before a mount and before `update_config`.** A
   refused configuration fails the operation with
   `KernelError::InvalidConfig { id, reason }`. On a mount nothing is
   mounted; on `update_config` the plugin, its dependents and its stored
   configuration are left as they were. A panicking check counts as a
   refusal. `restart` reuses the stored configuration and does not check it
   again.
3. **`config_schema` is removed** from `PluginManifest`. It was never read.
4. **Plugins check with the same code they apply.** `rah_api::config::parse`
   deserializes a configuration, `null` giving the default. Each static
   plugin parses with it, or a plugin-local `parse_config`, in both
   `check_config` and `apply`, and repeats in `check_config` the checks that
   need no I/O. Checks that need the environment or the file system (an
   API key variable, a directory that must exist) stay in `apply`, which can
   still fail.
5. **Hosts.** The process and WASM plugin hosts check their own launch
   configuration (`ProcessConfig`, `WasmConfig`). The configuration of the
   plugin program itself is not checked before it starts.
6. **`rah` at boot.** A refused configuration ends the boot with an error.
   For `policy-authority` it is reported as `PolicyInactive` ("refusing to
   run unguarded"), as before.

## Consequences

- A typo in a plugin's configuration no longer takes a running plugin down,
  and in particular no longer leaves tool calls unguarded while the Web UI
  rolls the policy back. Admin requests answer `400` with the reason.
- A configuration that passes the check can still fail in `apply`; the
  admin service and the Web UI keep their rollback for that case.
- At boot, an invalid configuration of any plugin stops `rah` with exit code
  2, where before that plugin was reported as not active and the rest
  started.
- The kernel grows by one trait method, one error variant and one helper.
