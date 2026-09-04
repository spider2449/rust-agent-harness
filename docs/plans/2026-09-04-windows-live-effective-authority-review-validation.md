# Windows Live Effective Authority Review Validation

## Result

Windows live-certified: PASS

Linux live certification: not established.

This document records Task 196A, the human-assisted Windows GUI validation of
the Effective Authority review UX. It does not replace the initial Task 196
result: Task 196 was INCONCLUSIVE because the implementation, build, and
fixture preparation succeeded but no desktop-control capability was available
to perform the required GUI interactions and visual checks. Task 196 made no
source or documentation changes, created no commit, and ran no CI. Its
fixtures were preserved and reused here.

## Source and certified runtime

- Live source baseline: `2741719a1034550f92c952acf81acb0f446ac229`
- `origin/master`: `2741719a1034550f92c952acf81acb0f446ac229`
- Task 195 exact-head CI: `33835056900` — PASS
- Certified Codex: `codex-cli 0.149.0`
- `codex.exe` SHA-256: `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- `codex-code-mode-host.exe` SHA-256: `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`
- Desktop binary: `target/release/rah-desktop.exe`
- Release `v0.15.0` state was not modified.

The release Desktop build succeeded. Desktop was launched with the certified
Codex directory first on `PATH`, and closed normally after validation.

## Fixture baselines and postconditions

Both existing fixtures were verified before use and were not recreated or
cleaned.

| Fixture | Baseline and post-run HEAD | Branch | Sentinel | Length | SHA-256 |
| --- | --- | --- | --- | ---: | --- |
| `authority-a` | `0b3db7270c7adf7b6be15c7755194bcddccd807d` | `master` | `sentinel-a.txt` | 18 | `9fe4f092dec0dd0d105290916c9d47c98398c2261a28a931338dd9935ddaa5c5` |
| `authority-b` | `494423300fe528fba75662a197db6337eae362e2` | `master` | `sentinel-b.txt` | 18 | `c5761713ebc313a02898576a38986edbfb5eca7ff8f6695eda0abb6130c77c59` |

For both repositories, pre-run and post-run status, worktree diff, and index
diff were empty. Branches, refs, HEADs, sentinel lengths, and hashes matched.

## Human GUI observations

### Startup and A disconnected

With no repository selected, the panel displayed Status `No repository
selected`, Repository `No repository selected`, Effective Tools `0`,
Unavailable `0`, and Reviewed commit `Not applicable`. No raw path,
fingerprint, `repo-context:`, or `rah_tool_` name appeared. The visible panel
controls were `Refresh Authority` and the collapsed `Advanced context`
disclosure; no authority or lifecycle controls were present. Two refreshes
caused no status, repository, connection, or Tool activity change.

After selecting A (`D:\RAH_TASK196_PRIVATE_PARENT\authority-a`) without
connecting, the panel displayed Status `Runtime disconnected`, Repository
`authority-a`, Binding `Unknown / unavailable`, Runtime `Runtime disconnected`,
Runtime source `Unknown / unavailable`, Effective Tools `0`, Unavailable `0`,
and Reviewed commit `Authorization revoked`. No full private parent path was
shown inside the panel. No generation values were visible. Two refreshes did
not start a connection or make the status Current.

### A current

After connecting without sending chat, the panel displayed:

- Status `Current`
- Repository `authority-a`
- Binding `Current`
- Runtime `codex`
- Runtime source `certified_side_by_side`
- Effective Tools `13`
- Unavailable `0`
- Reviewed commit `Authorization revoked`

No generation values were visible. The runtime source was sanitized and did
not contain an executable path.

The public Tool inventory was:

`echo`, `fs.read`, `repo.commit`, `repo.create-directory`,
`repo.create-file`, `repo.delete-file`, `repo.diff`, `repo.diff-staged`,
`repo.edit-files`, `repo.file-info`, `repo.patch`, `repo.rename-file`,
`repo.status`.

Tool cards showed source, effect, authority, dispatch permission, repository
binding, and runtime advertisement. `echo` was Desktop built-in, Execute,
Execute, no repository binding, and Advertised. Repository Tools were shown as
Repository host — Desktop repository, with their narrow read, observation, or
mutation effects and authorities, execute classification, repository binding,
and Advertised runtime state. No unavailable capabilities were listed.

The panel visibly distinguished Configured, Effective, and Advertised and
explained that requests remain subject to host permission and policy checks.
Three refreshes while A was Current caused no lifecycle, Tool, chat, inventory,
unavailable-list, or reviewed-commit change.

### A to B context switch

While A was connected/current, selecting B without reconnecting displayed:

- Status `Reconnect required`
- Warning: `This runtime inventory is not current for the selected context.`
- Repository `authority-b`
- Binding `Stale`
- Runtime `codex`
- Runtime source `certified_side_by_side`
- Effective Tools `13`
- Unavailable `0`
- Reviewed commit `Authorization revoked`

No generation values were visible. The prior Tool entries remained displayed,
but were not represented as current B runtime authority; `echo` was marked
`Not advertised / host effective only`. The private parent path did not appear.

Three refreshes while Reconnect required did not reconnect, change connection
state, produce Tool activity, or mutate repository B.

### Fresh B current

After a fresh connection, the panel displayed Status `Current`, Repository
`authority-b`, Binding `Current`, Runtime `codex`, Runtime source
`certified_side_by_side`, Effective Tools `13`, Unavailable `0`, and Reviewed
commit `Authorization revoked`. The Tool inventory was the same 13 public
Tools listed above. No generation values, private path, executable path,
fingerprint, or provider alias were visible.

Three refreshes while B was Current caused no reconnect, Tool activity, or
repository change. The panel remained stable.

## Control, privacy, and execution gates

The only control inside Effective Authority was `Refresh Authority`. No
Grant, Revoke, Authorize, Enable, Disable, Connect, Reconnect, Reload, or
Execute authority/lifecycle control was present.

The following were absent from Effective Authority: the private parent path,
the certified Codex executable directory, `repo-context:`, `rah_tool_`, token,
stderr, endpoint/URL, raw JSON, and review ID/digest/selector.

No chat prompt was sent and no Tool activity was visually observed during the
entire run. No live evidence JSONL was enabled or present, so no raw JSONL
tool-event count is claimed.

Desktop closed successfully using the normal application close.

## Repository change boundary

The only repository change from Task 196A is this evidence document. There are
no source, frontend, Cargo, ADR, permission, version, or dependency changes.

## Certification conclusion

All essential live gates passed: startup was non-current without a repository,
A transitioned from disconnected to Current, the public inventory and privacy
contract were visible, refresh was side-effect free, switching A to B required
reconnect and was not Current, fresh B became Current, no Tool executed, both
fixtures remained unchanged, and Desktop closed normally.

Task 196 Windows live certification: PASS.

Recommended next task: Task 197 — v0.16 Effective Authority Review Milestone
Audit. Do not begin it automatically.
