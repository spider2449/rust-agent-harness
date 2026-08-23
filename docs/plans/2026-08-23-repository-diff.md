# Task 061: deterministic shared diff foundation and `repo.diff`

Date: 2026-08-23
Status: Implemented; local deterministic validation complete, Ubuntu CI pending push

## Scope

This task implements only `repo.diff`, the fixed worktree-versus-index observer.
`repo.diff-staged` is deliberately deferred to Task 062: although command
selection would be a private `--cached` switch, unborn-HEAD behavior needs a
focused acceptance matrix and must not be folded into this task.

## Fixed execution envelope

`RepositoryObserver` owns the native Git executable, canonical/revalidated
repository root and identity, cleared/fixed Git environment, capture bounds,
aggregate 15-second timeout, and one existing exclusive per-repository RAH
lease. The lease is held once around all three commands and is released by
normal Rust scope unwinding on command, parser, or bounds errors.

The only fixed command shapes for this observer are:

```text
git --no-pager diff --raw -z --no-abbrev --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
git --no-pager diff --numstat -z --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
git --no-pager diff --patch --no-color --no-prefix --full-index --no-renames --no-relative --no-ext-diff --no-textconv --diff-algorithm=myers --no-indent-heuristic --inter-hunk-context=0 --unified=3 --ignore-submodules=all --submodule=short
```

The model request is exactly `{}`. It cannot select a baseline, flags,
pathspec, context, binary mode, rename behavior, executable, cwd, or child
environment. There is no generic Git runner or public diff-policy object.

The child environment is cleared and receives the Task 058 fixed configuration:
disabled system/global config; `core.fsmonitor=false` and
`core.untrackedCache=false` through `GIT_CONFIG_COUNT`; plus
`GIT_OPTIONAL_LOCKS=0` and `GIT_TERMINAL_PROMPT=0`. `--no-pager`,
`--no-ext-diff`, and `--no-textconv` additionally suppress pager, external
diff, and textconv paths. Tests configure hostile `diff.external` and a
`.gitattributes` textconv driver with a marker command and prove neither runs.

## Result and correlation contract

Success is one bounded JSON object:

```json
{
  "status":"ok",
  "consistency":"best_effort",
  "comparison":"worktree_to_index",
  "base":"index",
  "files":[{
    "old_path":{"encoding":"utf8","value":"src/lib.rs"},
    "new_path":{"encoding":"utf8","value":"src/lib.rs"},
    "change_kind":"modified",
    "old_mode":"100644",
    "new_mode":"100644",
    "binary":false,
    "added_lines":1,
    "deleted_lines":1,
    "patch":{"encoding":"utf8","value":"diff --git ..."}
  }]
}
```

Raw `-z` records create a private key from exact path bytes and normalized
change semantics. NUL-delimited numstat records must match that path set
one-to-one; duplicate, missing, malformed, or contradictory observations are
rejected without a partial result. The patch stream is intentionally opaque:
the implementation only splits verified top-level `diff --git ` sections and
requires a section for every raw record. It does not parse quoted patch paths
or hunk grammar. This preserves the opaque patch contract while keeping path
identity machine-readable.

Paths and patch bytes are tagged as UTF-8 when valid, otherwise exact base64.
No lossy decoding is used. Binary numstat `-`/`-` records produce
`binary:true`, null line counts, and `patch:null`; no binary patch or file
payload is returned.

Supported change kinds are `added`, `deleted`, `modified`, `type_changed`,
and `gitlink_changed`. Fixed `--no-renames` prevents rename/copy semantics.
Raw unmerged records fail closed rather than emitting a potentially misleading
patch. A conflict test proves this has no partial result. Submodules are
ignored and never recursed into.

## Semantics, limits, and consistency

The baseline is always index: staged-only content is absent, while a subsequent
unstaged edit is shown as the index-to-worktree delta. Untracked files are not
synthesized; use `repo.status` to discover them.

Each process stdout capture, raw parser, and numstat parser is capped at 1 MiB.
There may be at most 256 files, paths at most 4 KiB, individual patch sections
at most 256 KiB, and final normalized `ToolOutput` at most 1 MiB. Any capture,
count, record, section, correlation, or final serialization breach is an
explicit fail-closed error; nothing is truncated.

The observer claims **no intentional repository mutation**, not zero incidental
filesystem writes. Tests snapshot HEAD, refs, index, a tracked file, and an
untracked binary file before/after. The RAH lease only serializes RAH callers;
outside processes can still change the repository between the three commands.
Contradictory observations are detected and rejected where the raw/numstat/path
or patch-section invariants expose them. Mutually valid observations remain
`best_effort`; no snapshot or transaction claim is made.

## Deterministic evidence

Windows-native tests cover the closed schema and registry, clean/staged-only/
staged-plus-unstaged semantics, deletion, CRLF, Unicode/spaces paths, binary
NUL data, hostile external/textconv helper suppression, conflict rejection,
read-only snapshots, and parser bounds/malformed inputs.

Unix-only tests, compiled for Ubuntu CI, cover executable mode changes,
symlink replacement without dereference, case-distinct paths, newline/tab
paths, and a changed invalid UTF-8 filename rendered as base64. Ubuntu CI is
required before any Unix completion claim; no Unix live Codex claim is made.

## Architecture impact

No dependency edges, permission levels, ADRs, trusted-profile composition,
Generic Tool Bridge changes, mutation authority, or provider/Codex types were
introduced. Shared diff machinery remains crate-private.
