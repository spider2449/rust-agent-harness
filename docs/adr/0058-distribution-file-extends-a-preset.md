# ADR 0058: A Distribution File Can Extend A Preset

Status: Accepted

Date: 2026-10-29

## Context

A distribution file (`rah.toml`) is the whole plugin list. To change one
setting of a preset, such as the model's id or the loop's `max_rounds`, a user
runs `rah --init-config` and edits a copy of all fifteen plugins of a guarded
preset, policy included. From then on the copy no longer follows the preset: a plugin
or policy assignment a later release adds to the preset never reaches it.
`--preset` and `rah.toml` are either-or, and features (`--with`, `features`)
only cover the capabilities they were written for.

DeepSeek Harness, built on Cordis, composes its configuration in layers: a
base bundle, a mode bundle and the user's patch file, whose rows address the
base rows by id. The user writes only what differs. This ADR adopts the part
of that idea that fits RAH's static TOML: one layer over a built-in preset. It
does not adopt expressions in configuration, installable bundles or several
user layers.

## Decision

1. **`extends = "<preset>"`** at the top of a distribution file makes the
   file a list of changes to that built-in preset. A file without `extends`
   is the whole plugin list, as before.
2. **Changes are applied in a fixed order.** `remove = ["<id>", ...]` first
   drops preset plugins by instance id. Then each `[[plugin]]`:
   - whose instance id (its `id`, else its `use`) is in the preset changes
     that plugin in place, keeping its mount position, so the policy still
     mounts first;
   - with any other id is added before the frontend.
3. **A change merges its `config` into the preset's.** Tables merge key by key
   at every depth, and any other value (string, number, boolean, array)
   replaces the preset's. `bind` merges by service. An entry without `config`
   changes nothing in it. `replace = true` makes the entry's `config` and
   `bind` the plugin's whole configuration instead.
4. **Mistakes are refused at parse time**, before anything is mounted: an
   unknown preset, a `remove` id the preset does not have, a changed plugin
   whose `use` differs from the preset's (swap a plugin with `remove` and an
   addition), `replace` on a plugin the preset does not have, and `remove` or
   `replace` in a file without `extends`.
5. **`features` apply after the file**, as in a whole file, so a feature sees
   the composed plugins and keeps any policy entry the file wrote.
6. **`rah --show-config` shows the composition**: the extended preset, the
   removed ids, and for each plugin whether the file changed or added it.

`--preset` still ignores `rah.toml`, and `--init-config` still writes the
whole preset; its header says how to use `extends` instead.

## Consequences

- A user file can be a few lines and keeps following its preset across
  releases. In return, a preset change can change what a user file runs; the
  preset policy's assignments are among the things it follows.
- Removing `policy-authority` through `remove` is possible, as is leaving it
  out of a whole file. `rah` then warns `UNGUARDED` at start, as for any
  distribution without a tool policy.
- An addition whose id is mistyped is added as a new plugin rather than
  changing the intended one. `--show-config` marks it as added.
- Merged arrays replace rather than append, so adding one entry to an array
  (such as the policy's `allow`) means writing the whole array.
- Only `rah-cli` changes. The kernel, `rah-api`, the plugin protocol and every
  plugin are unchanged; plugins still receive one `config` value at mount.
