# Task 375 - Bounded Repository Structure Listing Implementation Contract

Date: 2026-09-20
Status: **CONTRACT - READY FOR TASK 376 IMPLEMENTATION**
Task type: research/design only

This document authorizes no Rust, production, version, dependency, permission,
ADR, tag, release, publication, or push change.

## Starting checkpoint and research conclusion

```text
HEAD          = 47f5de5 docs: define RAH v0.32 scope and authority roadmap
origin/master = 2f9bd83957a7cefbd83e2ca377fd2125b8376616
worktree      = clean
workspace     = 13 packages; version 0.31.0; edition 2024
```

The full authoritative source was read:
`docs/plans/2026-09-20-task-374-v0.32-scope-and-authority-roadmap.md`.
The sole v0.32 capability remains bounded structure browse:
`repo.list -> repo.search -> fs.read -> existing authoring/review Tools`.

Source research found no authority contradiction. The existing private
`RepositoryObserver` supplies the selected Git/repository identity, lease,
fixed command, output/time bounds, and revalidation. The existing nested
boundary policy supplies fail-closed repository boundary checks. `repo.search`
supplies the private NUL inventory normalization and current-worktree
classification. No new ADR or dependency is needed.

Task 369 proved that profile/effective-authority support alone does not ensure
active-repository Desktop registration; Task 376 must explicitly register and
test this Tool on that production path.

## Tool and authority decision

The canonical Tool is a separate first-party Tool:

```text
repo.list
```

Do not add a hidden third mode to the released `repo.search` `path`/`text`
contract. Structure browse has no query, directory-prefix navigation, and
file/directory kinds, so a separate Tool preserves the closed search contract.

Freeze:

```text
EffectClass       = ReadOnly
AuthorityCategory = RepositoryObservation
PermissionLevel   = Execute
repository_bound  = true
```

This is a **NARROW EXTENSION** only. `Execute` is not generic process, Git,
filesystem, mutation, network, credential, or persistence authority.

## Request and root contract

The closed JSON request is:

```json
{}
```

`{}` means the selected active repository root. A nested logical directory is
`{"path":"crates/rah-tools/src"}`. `path` is optional, but `{"path":""}`
is invalid and is not another root spelling. Unknown fields, non-object input,
null/non-string paths, and serialized requests over 4 KiB fail closed.

When present, `path` is 1..=1024 UTF-8 bytes, repository-relative and
slash-separated, with no leading/trailing slash, empty component, `.`, `..`,
ASCII-case-insensitive `.git`, backslash, colon, or NUL. The model supplies no
repository/worktree/member/root selector, executable, Git argv/pathspec, cwd,
environment, timeout, depth, recursion, wildcard, glob, or result limit.

Root always projects successfully, including an empty root. An existing
logical tracked directory returns direct children. A visible tracked file is a
sanitized not-a-directory error. A nonexistent, untracked-only, or ignored-only
logical prefix is a sanitized not-found/invalid-target error, not an ambiguous
successful empty result. Git does not track empty directories.

## Inventory and projection

Reuse exactly:

```text
git --no-pager ls-files --cached --deduplicate -z --full-name --
```

Git is an inventory source only. Do not substitute `git ls-tree`, `git status`,
`git grep`, `find`, `dir`, `Get-ChildItem`, `rg`, shell interpolation,
filesystem recursive enumeration, or model-controlled Git syntax.

The disclosure universe is `tracked + currently present + eligible + selected
active-worktree`. Clean, modified, staged-new, and tracked-ignore-matching
files are visible. Deleted, sparse-omitted, untracked, ignored-untracked,
untracked-only-directory, and ignored-only-directory paths are absent.

Projection is:

```text
fixed NUL inventory -> bounded UTF-8/logical validation
-> omit non-addressable records without lossy conversion
-> sort/deduplicate -> select prefix -> inspect current candidates no-follow
-> omit missing/non-regular candidates -> synthesize direct children
-> sort all entries by repository-relative UTF-8 byte order -> bound result
```

Directories are synthesized only from eligible visible tracked file paths. For
`crates/rah-tools/src/lib.rs`, the relevant parent listings synthesize
`crates`, `crates/rah-tools`, and `crates/rah-tools/src`. The requested prefix
is not returned; entries retain full safe repository-relative paths.

## Boundaries, links, nested repositories, submodules

Reuse `RepositoryNestedBoundaryPolicy` without weakening `repo.search`:

| Condition | Result |
| --- | --- |
| Ordinary tracked symlink | Omit as `non_regular`; never follow or synthesize target descendants. |
| Directory symlink or ambiguous symlink ancestry | Whole observation fails when the existing policy rejects it. |
| Tracked file reparse point | Omit as `non_regular`. |
| Junction, directory reparse, or ambiguous reparse ancestry | Whole observation fails. |
| Descendant `.git`/`.GIT` boundary | Whole observation fails; no partial entries or nested metadata. |

Gitlinks/submodules are never traversed and have no extra output kind:

```text
present Gitlink/submodule directory -> non_regular omission
absent Gitlink/submodule path       -> changed_or_missing omission
observable nested .git boundary     -> whole-observation failure
```

No child path is synthesized from submodule filesystem contents. Invalid UTF-8
tracked names are never replacement-converted or exposed; they contribute to
`non_addressable_path`. Malformed/empty NUL records are structural failures.
Existing Windows alias, device, ADS, colon, backslash, reparse, and private
`.git` protections remain authoritative.

## Output and bounds

Successful output is one JSON object in one `ToolOutput` JSON content item:

```json
{
  "status":"ok", "consistency":"best_effort", "path":null,
  "complete":true, "truncation_reason":null,
  "entries":[{"path":"Cargo.toml","kind":"file"},{"path":"crates","kind":"directory"}],
  "omitted":{"non_addressable_path":0,"non_regular":0,"changed_or_missing":0}
}
```

Nested `path` is the supplied safe relative string; root is JSON `null`, never
`""`. Kinds are exactly `file` and `directory`. No contents, snippets,
timestamps, sizes, OIDs, filesystem/private Git paths, registration data,
executable/profile paths, cwd/environment, raw stderr, handles, IDs,
generations, or browse-history persistence are visible.

Freeze these bounds: request 4 KiB; logical request/candidate path 1024 UTF-8
bytes; inventory stdout 4 MiB; records 100,000; entries 128; normalized output
128 KiB; total timeout 15 seconds. No limit is model-adjustable.

More than 128 direct eligible children returns the first 128 in total path
order with `status: "ok"`, `complete: false`, and
`truncation_reason: "result_limit"`; it must not silently truncate.

Inventory timeout/failure/overflow, record/output overflow, malformed NUL,
identity or Git replacement, nested-boundary rejection, unsafe reparse
condition, timeout, or serialization over 128 KiB returns sanitized
`ToolError` with no partial list. No retry, replay, rollback, or compensation.

## Currentness and isolation

Freeze this sequence:

```text
parse request -> acquire existing repository lease -> identity revalidate
-> Git executable/layout revalidate -> whole boundary validation
-> fixed inventory -> identity revalidate -> normalize/current-worktree classify
-> project direct children -> final identity revalidate -> bounded serialization
```

The existing observer retains fixed cwd/environment, pre-command Git checks,
boundary checks, output limits, remaining-time handling, and 15-second timeout.
The result is `best_effort`, not a transaction, snapshot, race-free TOCTOU
proof, global Git lock, or external-Git exclusion.

There is no repository selector. Active A exposes only A: never main/B paths,
private Gitdirs, common Git state, worktree registration/backlink/commondir,
or shared object storage. A-to-B switching publishes a fresh B-bound registry;
stale identity is rejected, not rebound.

## Profile, Desktop, Effective Authority, bridge

The explicit profile binding is:

```json
{"name":"repo.list","enabled":true,"permission":"execute","executable":"git","repository":"workspace"}
```

Reuse the existing profile version and observer shape. Do not add depth,
max-results, Git args, environment, cwd, root/worktree selectors, or hot
reload. Existing observer presence does not implicitly admit `repo.list`.

Task 376 must update profile composition and the active selected-repository
Desktop registry, and test both; no active repository means no bound Tool.
Effective Authority is exactly `ReadOnly / RepositoryObservation / Execute /
repository_bound=true`, host-owned and not inferred from provider/frontend
metadata.

HostExplicit remains exactly 11:

```text
fs.read; repo.file-info; repo.status; repo.diff; repo.diff-staged;
repo.create-branch; repo.patch; repo.edit-files; repo.create-file;
repo.delete-file; repo.rename-file
```

The invariant is `host_kind("repo.list") == None`; no HostInvocationKind,
category admission, or HostExplicit #12.

The ordinary route is:
`active repository -> ToolRegistry -> Generic Tool Bridge -> repo.list -> fixed
inventory -> bounded projection -> ToolOutput -> model continuation`.
No bridge redesign, provider bypass, or model repository routing is needed.

## Reuse/refactor decision

Task 376 should extract a private shared repository-observation primitive for
fixed inventory execution, bounded NUL parsing, logical UTF-8 validation,
sort/dedup, current-worktree eligibility, and omission accounting. It must not
be public. Tests must prove released `repo.search` remains unchanged in schema,
ordering, omission categories, current-worktree behavior, limits, output,
errors, boundary, and currentness. Do not duplicate security-sensitive parsing
or add a second Git inventory contract.

## Required deterministic tests

Task 376/audit must cover: root and nested listing; direct-child-only
semantics; deterministic ordering; file versus synthesized directory; >128
saturation; unknown field; invalid/absolute/empty path; backslash, colon, NUL,
`.`, `..`, `.git`, `.GIT`; nonexistent directory; tracked-file target;
clean/modified/staged-new/tracked-ignore-match visibility; untracked,
ignored, untracked-only, and ignored-only absence; deleted and sparse omission;
invalid UTF-8 without lossy conversion; symlink/junction/reparse behavior;
nested repository fail-closed; exact Gitlink behavior; identity and executable
replacement; inventory/record overflow; malformed NUL; timeout; output bound;
no partial structural result; privacy/redaction; no intentional mutation;
linked main/A/B isolation; A-to-B switching; explicit profile binding; Desktop
registration; exact Effective Authority; HostExplicit exactly 11;
`host_kind("repo.list") == None`; and Generic Tool Bridge dispatch.

Use deterministic barriers, not sleeps. Normal tests require no credentials,
internet, GPU, or live model.

## Windows certification matrix

Use a fresh disposable `main`, `linked-a`, `linked-b` fixture. Active A contains
`Cargo.toml`, `crates/alpha/src/lib.rs`, `crates/beta/src/lib.rs`, and
`docs/guide.md`, plus untracked-only/ignored-only directories, a tracked
ignore-match file, nested repo, junction/reparse fixture, and >128 direct
children. Include invalid/special paths and the frozen Gitlink case where the
host permits them.

Required evidence: root A only; `path: "crates"` returns alpha/beta only;
`path: "crates/alpha"` is direct-only; main/B sentinels and untracked/ignored
nodes are absent; private/common Git metadata is absent; >128 is deterministic;
switching to B yields fresh B-only structure; before/after repository state
shows no intentional mutation. A real `repo.list -> repo.search -> fs.read`
model attempt may be recommended under a separately adopted v0.32 baseline;
Task 375 does not refresh or redefine the historical Codex baseline, and
host-driven/bridge evidence is not model-selected inference evidence.

## Non-goals and stop triggers

Out of scope: recursive/model-depth trees; untracked/ignored discovery;
filesystem-wide listing; regex/fuzzy/semantic/vector search; persistent or
background indexes; directory content; generic shell/process/Git; network/MCP;
credentials/OAuth; cross-worktree/repository selectors; multiple active
repositories/union registry; Git worktree lifecycle; profile hot reload;
HostExplicit #12; versioning, release, tag, publication, and cleanup.

Stop and return to research if implementation requires a new PermissionLevel,
AuthorityCategory, ADR-worthy authority, broader filesystem disclosure,
untracked enumeration, persistent cache, external list/search executable,
cross-repository/worktree selector, model Git pathspec, network, mutation, or
an unjustified dependency edge.

## Artifact and next task

The sole artifact is this file:
`docs/plans/2026-09-20-task-375-bounded-repository-structure-listing-implementation-contract.md`.
Required validation is `git status --short`, `git diff --check`, and
`git diff --stat`. Changed-path review must prove documentation-only scope:
no Cargo, Rust, frontend production code, permissions, ADR, dependency,
version, or release changes. The exact next task is:

```text
Task 376 - Bounded Repository Structure Listing Production Implementation
```

## Final contract

```text
Tool: repo.list
{} -> selected active repository root
{"path":"tracked/directory"} -> logical tracked directory
direct children only; tracked/current eligible selected-worktree structure
directories synthesized from eligible tracked file paths
fixed tracked Git inventory reused from v0.31
ReadOnly / RepositoryObservation / Execute / repository_bound=true
repository-relative path; kind=file|directory; best_effort; deterministic; bounded
HostExplicit exactly 11; repo.list ineligible
new ADR: none; new dependency: none
next: Task 376 - Bounded Repository Structure Listing Production Implementation
```
