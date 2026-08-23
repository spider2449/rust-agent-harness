# Task 062: deterministic `repo.diff-staged`

Date: 2026-08-23

## Scope

This task adds only `repo.diff-staged`, a fixed host-selected read-only
index-versus-HEAD observer. It reuses Task 061's crate-private raw/numstat/patch
parser, strict correlation, tagged UTF-8/base64 paths, binary suppression,
bounds, redaction, process hardening, repository identity checks, and exclusive
repository lease. `repo.diff` retains its worktree-versus-index semantics.

The model request is exactly `{}`. `RepositoryDiffStagedTool` is the only new
public host-facing construction point; the comparison enum and shared runner are
crate-private. No generic Git execution, baseline/ref input, mutation authority,
permission level, profile composition, Generic Tool Bridge change, dependency,
or ADR is added.

## Fixed semantics and commands

The result schema is identical to `repo.diff`:

```json
{
  "status":"ok",
  "consistency":"best_effort",
  "comparison":"index_to_head",
  "base":"head",
  "files":[]
}
```

Each file retains tagged `old_path`/`new_path`, modes, normalized change kind,
binary flag, nullable numstat counts, and a bounded tagged patch or `null`.
The only semantic difference is the private `IndexVsHead` command selection:

```text
git --no-pager diff --cached --raw -z --no-abbrev --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
git --no-pager diff --cached --numstat -z --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
git --no-pager diff --cached --patch --no-color --no-prefix --full-index --no-renames --no-relative --no-ext-diff --no-textconv --diff-algorithm=myers --no-indent-heuristic --inter-hunk-context=0 --unified=3 --ignore-submodules=all --submodule=short
```

`--cached` compares HEAD to the index. Clean indexes are empty; staged adds,
modifications, deletes, mode changes, and symlink entries follow native Git
semantics. Unstaged-only and untracked files are absent, and an additional
unstaged edit after staging does not replace the staged result. Intent-to-add is
not inferred from worktree content: the normalized outcome follows the same
fixed native `git diff --cached` stream.

## Unborn HEAD and consistency

Before and after the three commands, a fixed
`git --no-pager rev-parse --verify -q HEAD` observes the baseline under the
same lease. A normal HEAD must remain byte-identical; an unborn HEAD is the
specific successful exit-one/no-output result. A change from unborn to born, a
change between normal HEAD identities, or a normal-to-unborn change rejects the
entire request without a partial result.

For an unborn HEAD, native Git's fixed `diff --cached` semantics compare the
index to its canonical empty tree and return `base:"empty_tree"`; an empty
index is empty. RAH does not hard-code SHA-1's empty-tree ID. This delegates
object-format selection to the fixed Git process, and the deterministic test
suite constructs an unborn SHA-256 repository to prove the observer does not
assume a 40-hex SHA-1 empty-tree object.

The RAH lease serializes RAH callers across baseline determination, raw,
numstat, patch, and the final baseline check. It is released by scope unwinding
on success, conflict/parser/bounds/process failure, and unborn observations.
External actors are not transactionally excluded: raw/numstat/patch
contradictions fail closed, changed HEAD is explicitly rejected, and mutually
consistent external changes remain `best_effort` rather than a snapshot claim.

## Safety and bounds

Unmerged raw records reject the logical diff with no partial patch. Binary
records have `binary:true`, null counts and patch, and never expose binary
payloads. NUL machine parsing preserves Unicode, spaces, tabs/newlines, and
Unix invalid UTF-8 paths through tagged encoding without lossy decoding.

Task 061 limits are unchanged: 256 files, 1 MiB raw/numstat/patch capture and
final serialized result, 256 KiB per patch section, and 4 KiB per path. Any
capture, parser, correlation, count, section, or serialization overflow fails
closed. The cleared/fixed environment plus `--no-pager`, `--no-ext-diff`, and
`--no-textconv` retains pager, external-diff, textconv, fsmonitor, alias,
credential/helper, and inherited-config-routing hardening.

Tests snapshot HEAD, refs, index, tracked content, and untracked content before
and after successful observation. The precise guarantee is **no intentional
repository mutation**, not zero incidental filesystem writes.

## Deterministic evidence

Windows-native coverage includes schema closure, clean/staged-plus-unstaged
separation, staged modification/addition/deletion, binary NUL content, CRLF,
Unicode/spaces, untracked exclusion, unborn SHA-1 and SHA-256 repositories,
empty unborn index, intent-to-add native semantics, hostile helper suppression,
conflict rejection, result file-count bounds, lease release, read-only snapshots,
and private changed-HEAD/unborn-to-born comparison behavior.

Unix-only coverage (for required Ubuntu CI) adds executable mode, symlink,
case-distinct, invalid UTF-8, and tab/newline staged paths. No Unix live Codex
validation is part of this task.
