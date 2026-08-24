# RAH v0.6 Repository Observer Milestone Audit

Status: release ready
Date: 2026-08-24
Audited baseline: `8cb96d81f1d572ed0f60bc09fc06cd42e3f142f4`
Released baseline: RAH v0.5.1
Codex live baseline: `codex-cli 0.149.0`

## Decision

**A. RELEASE READY.** The evidence justifies releasing RAH v0.6 as a
**Repository-aware read-only workflow inspection** milestone. There is no known
v0.6 blocker. It adds only the four bounded observers below, composed through
the existing trusted profile, and their use beside the existing guarded
`repo.patch` capability. It does not add mutation, commit/history, networking,
plugin lifecycle, or persistence authority.

Live-Codex support is verified on Windows with exactly `codex-cli 0.149.0`.
Ubuntu provides required deterministic Git/observer CI evidence, not Unix live
Codex validation.

## Final capability and authority scope

| Capability | Closed request | Final semantics |
| --- | --- | --- |
| `repo.file-info` | `{"path":"<UTF-8 repository-relative path>"}` | One validated path's repository semantic state. It returns no file content; an eligible direct regular file of at most 1 MiB receives only byte length and SHA-256. |
| `repo.status` | `{}` | Repository-wide normalized porcelain status: tracked, staged, unstaged, and untracked state; no ignored files or file contents. |
| `repo.diff` | `{}` | Worktree versus index. It excludes staged-only and untracked changes. |
| `repo.diff-staged` | `{}` | Index versus HEAD, or the empty tree for an unborn HEAD. It excludes unstaged changes. |

All four definitions require `PermissionLevel::Execute`. This is only the
existing outer subprocess gate. It is not generic Git, filesystem, or mutation
authority: each capability has its own fixed host policy. A model request is
never authorization.

The model cannot select the Git executable, arbitrary Git argv, cwd,
environment, refs/baselines, textconv, external diff, or repository. The host
selects and revalidates the native Git executable and trusted repository root.
No arbitrary filesystem read or write authority is introduced.

`repo.patch` may be composed beside all four observers, but authority does not
merge: `repo.patch` retains bounded worktree mutation authority; observers
retain fixed read-only observation. Equal `Execute` permissions do not make
their authority equal.

## ADR and profile conclusions

No ADR was needed or added. ADR 0010 remains index-mutation-only, ADR 0011
remains the trusted capability-profile authority-composition boundary, and ADR
0012 remains worktree-content-mutation authority. Observers are not extensions
of ADR 0010 or ADR 0012.

Task 063 reused `profile_version = 1` and the existing closed
`capabilities[]` schema. There is no top-level observer schema. Each observer
binds existing symbolic Git-executable and repository resources. Static
validation reports configured/unregistered without observer construction or
execution; effective composition constructs, registers, and validates each
tool in a fresh registry without executing it. Redacted inventory contains
logical capability/resource identities and state, never raw host paths.

The only public API addition in Tasks 057–065 is the narrow host-facing
`RepositoryObserverProfile` used for profile composition. There is no
model-visible/public protocol authority expansion, profile-version bump, new
crate, crate-topology change, dependency edge, or permission-level change.

## Fixed commands and hardening

Every Git invocation is direct argv under `--no-pager`, a fixed canonical
repository cwd, a cleared environment, revalidated executable/repository
identity, bounded capture, timeout, and the existing exclusive RAH repository
lease.

- `repo.status`: fixed `status --porcelain=v2 -z --untracked-files=normal
  --ignored=no --no-renames --ignore-submodules=all`.
- `repo.file-info`: fixed path-scoped `ls-files --stage -v -z --full-name
  --no-abbrev`, `rev-parse --verify -q HEAD`, `ls-tree -z -l HEAD`, and
  porcelain-v2 NUL status observations. The sole literal path follows `--`
  under `--literal-pathspecs`.
- `repo.diff`: fixed raw, numstat, and patch worktree-versus-index commands.
- `repo.diff-staged`: the same correlated raw/numstat/patch family with
  `--cached`; unborn HEAD normalizes to `base:"empty_tree"`.

Diff commands also fix `--no-color`, `--no-ext-diff`, `--no-textconv`,
`--no-renames`, `--ignore-submodules=all`, and the designed submodule handling.
The process environment disables system/global config, inherited HOME/XDG/PATH,
pager, external-diff and credential/proxy leakage, fsmonitor, untracked cache,
optional locks, and terminal prompting. Deterministic hostile-helper fixtures
prove external-diff/textconv-related helpers are not invoked.

## Confinement, encoding, and output bounds

Repository identity is captured and revalidated from the host-selected root and
`.git` metadata. `repo.file-info` accepts exactly one nonempty UTF-8 logical
path: at most 1,024 bytes, no NUL, absolute/drive/UNC form, backslash, colon,
`.`/`..`, or case-insensitive `.git` component. Link/reparse ancestry is
rejected before a direct digest read, so symlink handling does not create
external filesystem read authority.

Machine outputs use NUL framing where applicable. Model-visible paths are
either validated UTF-8 or exact tagged base64; decoding is never lossy.
Invalid UTF-8 Unix paths have deterministic CI coverage, while `repo.file-info`
selection intentionally remains UTF-8 only. Status/diff do not return binary
content. Binary diff entries are structural metadata with `patch: null`; NUL
in files or names does not trigger text conversion or break the NUL-framed
semantic contract.

| Tool | Bound |
| --- | --- |
| `repo.file-info` | 4 KiB request; 1,024-byte path; 1 MiB eligible digest input; 128 KiB result. |
| `repo.status` | 10,000 entries; 4 MiB capture/result; 4 KiB path; 16 KiB record. |
| Diff observers | 256 files; 1 MiB total capture/final result; 256 KiB per patch section; 4 KiB path. |

Timeout, capture/result overflow, malformed NUL records, correlation mismatch,
identity contradiction, unsupported records, and unmerged diffs fail closed:
they produce no silently truncated semantic result or partial misleading patch.

## Sparse, conflict, and consistency model

`repo.file-info` reports `normal`, `skip_worktree_present`, `sparse_omitted`,
`skip_worktree_unknown`, `not_tracked`, or `conflicted`. `repo.status` does not
overclaim per-path sparse state and reports `sparse_index_flags:"not_enumerated"`.
Both diffs fail closed on conflicts; `repo.diff-staged` also detects HEAD races
and supports unborn HEAD.

Every successful observer reports `consistency:"best_effort"`. The RAH lease
serializes RAH repository operations but creates neither a snapshot transaction
nor a cross-process lock. Detectable contradictions fail closed; mutually
consistent external races between Git observations can remain undetectable.
This is a documented limitation, not a v0.6 release blocker.

The exact read-only release claim is **no intentional repository mutation**.
It is deliberately not a claim of zero filesystem writes: Git or the platform
may make incidental metadata writes despite hardened observer configuration.

## Bridge and live-Codex evidence

Task 064 deterministically exercised the real profile -> composer -> fresh
registry -> fake app-server Generic Tool Bridge chain for all four canonical
observers. It verified deterministic aliases, Execute admission, denial for
None/Read/Write before observer entry, success, deduplication, cancellation
before entry, no replay, response redaction, and `repo.patch` route isolation.
It changed no production bridge behavior.

Task 065 then exercised the native app-server chain on Windows:

```text
TrustedStaticProfile -> real composer -> fresh ToolRegistry
-> Generic Tool Bridge -> native Codex app-server -> observers -> Completed
```

At exact `codex-cli 0.149.0`, three fresh successful runs observed these private
aliases: `repo.diff -> rah_tool_0`, `repo.diff-staged -> rah_tool_1`,
`repo.file-info -> rah_tool_2`, and `repo.status -> rah_tool_3`. Each observer
was requested, started, finished, and invoked exactly once. The terminal marker
was exactly `RAH_REPOSITORY_OBSERVERS_LIVE_OK`.

The live fixture proved unchanged HEAD, refs, raw index bytes, tracked and
untracked fixture files, staged diff, and unstaged diff. Codex-owned shell,
filesystem, MCP, process, network/web, image, apps, and approvals were disabled,
so the evidence proves RAH observer routing rather than alternate Codex access.
The app-server and Git children were reaped; no MCP/Plugin child remained; the
temporary profile and repositories were removed. Windows lock/lifecycle cleanup
was explicitly checked.

## Platform and CI evidence

Windows evidence includes deterministic tests and the three fresh live-Codex
observer runs. Ubuntu/Linux evidence is deterministic: Git/observer behavior,
invalid UTF-8 paths, executable mode, symlink semantics, case-sensitive paths,
and newline/tab paths. It does not establish Unix live-Codex support.

Required CI runs recorded for this milestone are successful: Task 060
`32617301678`, Task 061 `32619701227`, Task 062 `32620596125`, Task 063
`32681777540`, Task 064 `32682605820`, and Task 065/current audited HEAD
`32683776776`. The Task 065 Ubuntu run completed with success.

## Product workflow and remaining debt

v0.6 enables an agent to inspect repository state, inspect a specific file's
eligibility/state, inspect unstaged changes, inspect staged changes, use the
existing guarded `repo.patch` for a bounded tracked-file edit, and verify the
result through observers.

Still absent: file creation; deletion/rename; multi-file transactions; generic
patches/hunks; Git commit/history authority; network Git; and session
persistence.

| Item | Classification | Disposition |
| --- | --- | --- |
| External-actor races / best-effort observation | B. Fold into release validation | State the limitation and preserve fail-closed checks. |
| No Unix live-Codex validation | B. Fold into release validation | State Windows-only live scope; do not claim Unix live support. |
| Per-observer private `RepositoryObserver` envelope | C. Post-v0.6 maintenance | Consider internal consolidation only with unchanged boundaries. |
| Prior repo.patch raw-NUL/request-BOM fixture debt | C. Post-v0.6 maintenance | Does not weaken observer evidence. |
| Sparse/skip-worktree test depth | C. Post-v0.6 maintenance | Current explicit contracts and deterministic coverage are sufficient. |
| Mid-subprocess cancellation/disconnect instrumentation | D. Future capability research | Pre-entry and generic bridge behavior are covered; do not invent rollback/replay. |

None is a release blocker.

## Recommended next task

Task 067 — RAH v0.6.0 Release Preparation: bump workspace crates and lockfile
metadata to 0.6.0, update CHANGELOG and release gate/docs as needed, rerun all
deterministic gates and the exact `codex-cli 0.149.0` live observer gate, verify
a clean tree, and commit release preparation. Do not create or move a v0.6.0 tag
until that commit and required CI are green.
