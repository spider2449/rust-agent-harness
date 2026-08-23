# Task 060 plan: deterministic repository status

Date: 2026-08-23
Status: Implemented; deterministic local validation pending final workspace gate

## Scope

Task 060 adds only the canonical `repo.status` observer. It reuses Task 059's
private repository-observer foundation: a host-selected native Git executable,
canonical/revalidated repository identity, cleared and fixed Git environment,
bounded supervised process capture, timeout, and the existing exclusive
per-repository RAH lease. No generic Git executor, profile/bridge change,
mutation capability, permission level, ADR, `repo.diff`, or `repo.diff-staged`
is introduced.

The existing Windows fallback identity was hardened to use the native volume
serial number and file index, eliminating a rapid directory-replacement gap in
the shared repository identity guards (including the pre-existing mutation
fixture). This uses the already-present `windows-sys` edge and adds no
authority or public API.

## Fixed command and request

The model-visible request is exactly `{}`. The fixed direct-argv command is:

```text
git --no-pager status --porcelain=v2 -z --untracked-files=normal --ignored=no --no-renames --ignore-submodules=all
```

The inherited environment is cleared. The child receives only the Task 058
Git hardening values: disabled system/global configuration, disabled
`core.fsmonitor` and `core.untrackedCache`, `GIT_OPTIONAL_LOCKS=0`, and
`GIT_TERMINAL_PROMPT=0`. This prevents inherited aliases, pager, credentials,
external environment overrides, and fsmonitor helper behavior. `status` does
not use external diff or textconv; a hostile local `diff.external` marker is
also covered as defense in depth.

## Normalized result

Successful output is one closed JSON object:

```json
{
  "status": "ok",
  "consistency": "best_effort",
  "entries": [{
    "path": {"encoding": "utf8", "value": "src/lib.rs"},
    "previous_path": null,
    "tracked": true,
    "index_state": "modified",
    "worktree_state": "unmodified",
    "conflict_state": "none",
    "submodule_state": "none",
    "head_mode": "100644",
    "index_mode": "100644",
    "worktree_mode": "100644",
    "stages": []
  }],
  "sparse_index_flags": "not_enumerated"
}
```

`index_state` and `worktree_state` use `unmodified`, `added`, `modified`,
`deleted`, `renamed`, `copied`, `type_changed`, `unmerged`, or `untracked`.
Unmerged records retain `unmerged` and use one of `both_added`,
`both_deleted`, `added_by_us`, `deleted_by_them`, `added_by_them`,
`deleted_by_us`, or `both_modified`; their stage modes are retained in
`stages`. Ordinary records retain the porcelain modes. Rename/copy parsing is
defensive only; the fixed `--no-renames` command does no host-side inference.

Paths are parsed only as NUL-delimited bytes and rendered with the shared
tagged representation: valid UTF-8 is `utf8`, otherwise exact bytes are
base64. No content, decoded binary data, host path, executable path,
environment, repository identity, or raw stderr is returned.

`--untracked-files=normal` includes an untracked directory as one directory
entry and excludes its children. Ignored output is disabled; a surprise
ignored record is a closed parser error rather than a partial result.

## Bounds and parser

The status process and normalized result are independently capped at 4 MiB,
with a 10-second timeout and at most 10,000 entries. Each path is capped at
4 KiB and each porcelain record at 16 KiB. Capture overflow, timeout,
non-success, malformed/truncated NUL records, malformed fields/XY/modes/OIDs,
unexpected headers, unexpected ignored records, and normalization overflow all
fail closed with no partial semantic list.

The private parser supports porcelain-v2 `1`, defensive `2`, `u`, and `?`
records. It recognizes `!` and headers solely to reject them deterministically
when the command contract says they cannot occur.

## Sparse, consistency, and read-only semantics

Porcelain status omits clean index entries and cannot accurately enumerate
skip-worktree/assume-unchanged state for all paths. Therefore this result says
`sparse_index_flags:"not_enumerated"`; `repo.file-info` remains the detailed
per-path sparse observer. The output is explicitly `best_effort`: the RAH
lease serializes RAH repository operations, not external Git actors, and does
not form a snapshot transaction.

The precise read-only claim is **no intentional repository mutation**. Tests
snapshot HEAD, refs, index bytes, one tracked worktree file, and untracked
binary bytes before/after observing. `GIT_OPTIONAL_LOCKS=0` is used, but Git,
the OS, or future Git versions are not claimed incapable of incidental writes.

## Deterministic tests

Windows-native temporary repository tests cover clean schema/registry input,
staged plus unstaged CRLF modification, staged and unstaged deletions, binary
modification without content exposure, space/Unicode names, untracked regular
file and `normal` untracked directory behavior, ignored exclusion, conflict
normalization, hostile fsmonitor/diff helper markers, read-only snapshots, and
lease serialization/release. Private parser tests cover every record form,
all seven conflict pairs, invalid UTF-8 base64 paths, newline/tab paths,
count/record bounds, and malformed output.

Unix-only tests are cfg-gated for executable-bit transition, symlink state,
case-distinct paths, newline/tab parsing, and invalid UTF-8 tagged paths. They
must be verified by Ubuntu CI after push; this task makes no Unix live-Codex
claim.

## Known limitations

The result has no branch metadata, rename inference, ignored entries, generic
path selection, content reads, consistency token, shared/read lease, or
cross-process locking. It is a bounded state observer, not a transaction or
OS sandbox.
