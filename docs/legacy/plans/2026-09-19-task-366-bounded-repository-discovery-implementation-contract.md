# Task 366 — Bounded Repository Discovery Implementation Contract

Date: 2026-09-19
Status: **RESEARCH / DESIGN COMPLETE — NO RUST IMPLEMENTATION**
Parent checkpoint: Task 365, `9e6caf3`
Released production baseline: RAH v0.30.0

## Final verdict

Task 366 freezes the v0.31 primary capability as one canonical first-party
Tool:

```text
repo.search
```

It has two closed modes, `path` and `text`, and is a narrow extension of the
existing repository-observer family. The implementation target is the active,
host-selected repository member only. Task 366 changes no Rust, tests,
dependency, ADR, permission, persistence, release, or live-certification
state.

The product loop is:

```text
active selected repository
  -> bounded tracked-file discovery
  -> safe repository-relative path / matching line numbers
  -> existing fs.read, repo.file-info, or authoring tools
```

This is repository-bounded discovery, not generic filesystem discovery and not
generic Git search.

## Authority decision

Classification: **NARROW EXTENSION**.

`repo.search` uses the existing repository observation boundary:

```text
EffectClass       = ReadOnly
AuthorityCategory = RepositoryObservation
repository_bound  = true
PermissionLevel   = Execute
```

`Execute` remains only the existing outer gate for a fixed, host-selected,
read-only native Git inventory subprocess. It does not grant generic process,
shell, Git, filesystem, mutation, staging, commit, network, credential, or
persistence authority.

No new authority ADR is created. ADR 0027 remains authoritative for one active
host-selected repository composition, and ADR 0029 remains authoritative for
linked-worktree identity and selected-member isolation. If implementation
requires any of the following, Task 367 must stop and reopen authority
research before code is written:

- persistent or background indexing;
- untracked or ignored broad discovery that depends on ambient Git config;
- an external search executable or model-selected Git syntax;
- Git object-store, historical, sibling-worktree, or cross-repository search;
- symlink or reparse traversal;
- network, credentials, a new selector, or new lifecycle authority.

## Canonical Tool surface

There is one Tool, not a `repo.list`/`repo.tree`/`repo.grep`/`repo.find`
combination. A required non-empty query binds disclosure and output to an
explicit discovery intent and avoids an unconditional repository tree dump.

Conceptual closed schema:

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["mode", "query"],
  "properties": {
    "mode": {"type": "string", "enum": ["path", "text"]},
    "query": {"type": "string"},
    "path_prefix": {"type": "string"}
  }
}
```

Rust-side validation is authoritative because JSON Schema character limits are
not security boundaries:

| Field | Contract |
| --- | --- |
| serialized request | `<= 4096` bytes |
| `mode` | exactly `path` or `text` |
| `query` | `1..=256` UTF-8 bytes; no NUL, CR, or LF |
| `path_prefix` | optional; `1..=1024` UTF-8 bytes; repository-relative logical path |

`path_prefix` must be slash-separated and must reject absolute/root/prefix
forms, backslashes, colons, NUL, empty components, `.` and `..` components,
and any ASCII-case-insensitive `.git` component. It is a literal prefix:

```text
candidate == prefix || candidate starts with prefix + "/"
```

The model can select only `mode`, `query`, and `path_prefix`. It cannot select
the repository, worktree, member, root, Git executable, argv, timeout,
environment, or search limits.

## Search universe and Git role

v0.31 searches **tracked, currently present, ordinary regular files in the
selected active worktree only**. It excludes untracked files, ignored files,
empty directories, symlink targets, reparse targets, submodule contents,
nested repositories, Git metadata, sparse-omitted files, and deleted tracked
paths.

The fixed host-selected inventory command is:

```text
git --no-pager ls-files --cached --deduplicate -z --full-name --
```

It runs with the exact selected repository root as cwd, the existing
`repository_observer_environment`, the existing fixed output/time policy, and
pre/post identity revalidation. `--cached` supplies index-tracked paths,
`-z` provides byte-safe NUL records, and `--deduplicate` removes duplicate
filename records from multi-stage index entries. Query, prefix, mode, and all
matching semantics remain host-owned Rust logic; none is passed as Git argv.

This tracked-only decision deliberately avoids the ambient-global-ignore
ambiguity created by untracked enumeration. `GIT_CONFIG_NOSYSTEM=1`,
platform-appropriate `GIT_CONFIG_GLOBAL` isolation, and `GIT_OPTIONAL_LOCKS=0`
remain mandatory. Git is an inventory source only; v0.31 does not use
`git grep`, `rg`, `grep`, `find`, a regex crate, a filesystem walker
dependency, or a semantic/vector index.

## Candidate eligibility

Inventory records become candidates only when they satisfy the existing RAH
logical-path contract:

- valid UTF-8, with no lossy conversion;
- at most 1024 UTF-8 bytes;
- relative, slash-separated path;
- no forbidden component and no `.git` component.

The selected-root filesystem observation must then confirm that the candidate
exists as an ordinary regular file, is not a symlink, is not a junction or
reparse point, and remains inside the validated selected repository boundary.
Invalid UTF-8 tracked names are omitted on Unix rather than exposed or
lossily converted.

For `text`, the current selected-worktree bytes must additionally be at most
1 MiB, bounded-readable, valid UTF-8, and NUL-free. Files failing these tests
are omitted under an aggregate category. `path` mode does not read file
content.

Search observes current worktree bytes, not HEAD blobs, index blobs, the
object store, or another worktree. A staged file whose worktree has changed
is searched using its current worktree bytes. Sparse-omitted and deleted
tracked entries are omitted because no eligible selected-worktree file exists.

## Matching semantics

`path` mode performs a literal, case-sensitive UTF-8 string/byte substring
match against normalized repository-relative paths. It has no regex, glob,
fuzzy, locale-folding, filesystem-case-folding, or Unicode-normalization
semantics.

`text` mode performs a literal, case-sensitive, single-line match in current
worktree bytes. It never matches across line boundaries. CRLF and LF both
advance the 1-based logical line number at LF boundaries. A source line is
reported at most once even when the query appears multiple times on that line.

No source snippets are returned. Search discovers paths and line numbers;
`fs.read` remains the content-retrieval boundary.

## Ordering and output

After NUL inventory parsing, the host must validate paths, sort by normalized
UTF-8 path bytes, defensively deduplicate, apply `path_prefix`, and then
evaluate the requested mode. Filesystem enumeration order is never observable.

Path matches are ascending by path. Text matches are ascending by path and
then line number. Both modes return one normalized JSON object. Conceptually:

```json
{
  "status": "ok",
  "consistency": "best_effort",
  "mode": "path",
  "complete": true,
  "truncation_reason": null,
  "matches": [],
  "omitted": {
    "non_addressable_path": 0,
    "non_regular": 0,
    "changed_or_missing": 0
  }
}
```

Text mode adds `oversized` and `non_text` omission counters and returns
`matches` shaped as `{ "path": string, "lines": [number] }`. Omission
categories are aggregate only; no skipped-file detail is needed. Output must
not contain absolute paths, private/common Git paths, executable paths,
environment, cwd, generations, member IDs, registry identities, raw stderr,
or the query merely for reflection.

## Hard bounds

Task 367 must implement exactly these initial bounds unless a separately
authorized contract revision changes this plan:

| Bound | Value |
| --- | ---: |
| serialized request | 4 KiB |
| query | 256 UTF-8 bytes |
| path prefix | 1 KiB |
| Git inventory stdout | 4 MiB |
| inventory records | 100,000 |
| candidate logical path | 1 KiB |
| per text file | 1 MiB |
| aggregate text bytes read | 16 MiB |
| path results | 128 |
| text-result files | 64 |
| matching lines per file | 8 |
| total matching lines | 128 |
| normalized ToolOutput | 128 KiB |
| total search timeout | 15 s |

The existing observer stderr audit-tail bound remains in force.

Structural/process violations fail closed with a sanitized ToolError and no
partial match list. This includes inventory overflow or malformed NUL records,
inventory record overflow, Git timeout/process overflow, invalid identity or
nested-boundary state, and unexpectedly oversized normalized output.

Search-evaluation limits produce a deterministic successful partial result:
`status = ok`, `complete = false`, and `truncation_reason` equal to
`result_limit` or `scan_byte_limit`. A non-match with `complete = false` is
not proof of absence. There is no automatic retry or replay.

## Lease, identity, and boundary sequence

The operation follows the existing repository-observer lifecycle:

```text
parse request
  -> acquire existing repository lease
  -> revalidate repository identity
  -> revalidate Git-layout semantics
  -> validate the whole repository boundary
  -> run fixed ls-files inventory
  -> revalidate repository identity
  -> normalize, sort, deduplicate, and prefix-filter candidates
  -> perform bounded path/text search
  -> final repository identity revalidation
  -> produce bounded normalized output
```

The lease serializes RAH-owned operations only. It does not lock external
Git, editors, or other filesystem clients. The result is therefore
`consistency = best_effort`; it makes no transaction, snapshot, race-free
TOCTOU, global lock, rollback, replay, or OS-sandbox claim.

An observable nested `.git` boundary rejects the whole search. The operation
does not silently recurse, skip and continue, or search nested content. The
selected root's `.git` metadata is never an inventory/result candidate.

For linked worktrees, ADR 0029 applies unchanged: active A searches only A's
selected worktree files. It never searches main, sibling B, shared object
storage, private gitdir, common gitdir, or worktree registration data. Shared
common Git state is relationship evidence, not sibling authority.

## Authority and profile integration contract

The canonical metadata for `repo.search` is:

```text
EffectClass       = ReadOnly
AuthorityCategory = RepositoryObservation
repository_bound  = true
PermissionLevel   = Execute
```

It may be advertised through the existing sanitized Effective Authority
observer capability shape. It must not expose implementation details, host
paths, search limits, or identity generations.

Trusted Profile integration reuses `profile_version = 1` and the existing
closed observer capability form:

```json
{
  "name": "repo.search",
  "enabled": true,
  "permission": "execute",
  "executable": "git",
  "repository": "workspace"
}
```

The capability must be explicitly configured. Existing `repo.status` or any
category membership must not implicitly grant `repo.search`. Extra fields such
as `workspace`, `max_bytes`, `cwd_resource`, arbitrary argv, environment, or
search limits remain rejected. Provider metadata cannot create or escalate
this capability.

The Generic Tool Bridge remains unchanged:

```text
model
  -> Generic Codex Tool Bridge
  -> ToolCall repo.search
  -> ToolRegistry
  -> authorized dispatch
  -> RepositorySearchTool
  -> fixed inventory and host matching
  -> ToolOutput
  -> model continuation
```

`repo.search` is explicitly ineligible for HostExplicit. The production
HostExplicit set remains exactly these 11 names:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
repo.create-file
repo.delete-file
repo.rename-file
```

Task 366 does not combine search with HostExplicit `repo.create-directory`
eligibility #12.

## Deterministic implementation evidence required by Task 367/368

The implementation and independent audit must exercise real production paths,
not helper-only assertions, for at least:

| Fixture or condition | Required result |
| --- | --- |
| clean tracked UTF-8 file | path/text match |
| modified tracked file | current worktree content is searched |
| staged-new tracked file | searchable |
| untracked or ignored untracked file | absent |
| tracked file matching ignore pattern | searchable |
| deleted or sparse-omitted tracked file | omitted |
| Unicode path/content | deterministic match |
| CRLF and LF | correct line numbers |
| empty/oversized query or prefix | reject |
| regex-looking query `.*[]()` | literal only |
| case variation | case-sensitive |
| invalid UTF-8 path/file | omitted, never lossy |
| binary/NUL or oversized file | text omission |
| symlink, junction, or reparse target | not followed / fail closed at boundary |
| nested `.git` or selected `.git` metadata | whole search failure / never exposed |
| huge or malformed inventory | closed failure |
| result or scan-byte saturation | deterministic `complete=false` |
| Git timeout | no partial result |
| replaced root/Git identity | reject |
| external worktree change | best-effort result, no retry |
| main / linked A / linked B | selected-root isolation |
| shared common Git directory | no sibling disclosure |
| HostExplicit | exactly 11; search ineligible |
| Effective Authority | ReadOnly / RepositoryObservation |
| Trusted Profile | explicit `repo.search`, execute permission |
| provider metadata | cannot create authority |

Every deterministic and Windows live fixture must capture before/after
worktree bytes/state, index bytes/state, HEAD, selected branch ref, and
relevant sibling state. The evidence must demonstrate no intentional mutation.
It must not overclaim provably zero filesystem writes; existing observer
wording about incidental OS/Git effects remains the limit.

The Windows Task 369 fixture must contain `main`, `linked-a`, and `linked-b`
with unique sentinels. With linked A active, A's sentinel is discoverable and
B/main sentinels are not. The gate must also cover path mode, text mode,
prefix narrowing, ordering, untracked exclusion, binary/oversized omission,
nested/reparse failure, identity privacy, and the marker:

```text
RAH_V031_REPOSITORY_SEARCH_LIVE_OK
```

Because this capability is directly model-facing, Task 369 should attempt the
real Generic Tool Bridge with the project baseline `codex-cli 0.149.0`. A
missing baseline tool must be recorded as an explicit release-gate decision;
ambient newer Codex, mocked Git, direct state injection, or helper-only calls
do not establish live model-path proof.

## Explicit non-goals

v0.31 does not provide directory-tree dumps, unconditional listing, regex,
glob, fuzzy, semantic, vector, or persistent search; background indexing;
untracked or ignored search; binary or invalid-UTF-8 content search; content
over 1 MiB; symlink/reparse traversal; submodule/nested recursion; Git object
database or historical search; sibling/cross-worktree/cross-repository search;
generic Git, `git grep`, `rg`, `grep`, `find`, shell, process, network,
credentials, mutation, staging, commit, retry, replay, rollback, or a new
selector/authority category.

## Expected Task 367 source surface

The following are implementation leads only; Task 366 does not modify them:

```text
crates/rah-tools/src/repository_search.rs
crates/rah-tools/src/repository_observer.rs
crates/rah-tools/src/lib.rs
crates/rah-tools/src/trusted_profile.rs
crates/rah-profile-composition/src/lib.rs
crates/rah-desktop/src/effective_authority.rs
HostExplicit and Generic Bridge deterministic tests
README / architecture / release documentation as authorized by Task 367 scope
```

No new Cargo dependency is expected and no public generic search API is
required.

## Source-review anchors

Task 365's roadmap, the existing `RepositoryObserver` and Git-layout
environment, observer Tool modules, Trusted Profile parser/composition,
Effective Authority classification, HostExplicit allowlist, ADR 0027, and
ADR 0029 were reviewed as the authority/source-of-truth baseline. They
collectively establish the fixed host Git boundary, repository lease and
identity/currentness sequence, active-only linked-worktree semantics, existing
observer permission shape, profile version, privacy boundary, and exact
HostExplicit 11-name set.

## Task sequencing and closure

```text
Task 366  contract complete (this plan)
    -> Task 367  bounded production implementation
    -> Task 368  independent deterministic/security/currentness audit
    -> Task 369  Windows live certification and model-path gate
    -> Task 370  v0.31 milestone audit and release preparation
    -> separately authorized publication task
```

Task 366 closure is documentation-only. Required checks are:

```text
git diff --check
cargo metadata --no-deps --format-version 1
```

The metadata check must continue to show 13 workspace packages, version
`0.30.0`, and edition `2024`. No commit, push, tag, release publication,
Rust implementation, dependency change, live certification, or adjacent
HostExplicit work is authorized by this task.

## Final contract summary

```text
CAPABILITY             repo.search
MODES                  path, text
AUTHORITY              narrow extension
EFFECT                 ReadOnly
CATEGORY               RepositoryObservation
PERMISSION             existing PermissionLevel::Execute
SEARCH UNIVERSE        tracked, present, ordinary regular files
ROOT                   active host-selected worktree only
MATCHING               literal, case-sensitive, host-owned Rust
GIT ROLE               fixed tracked-file inventory only
MODEL-SELECTED ARGS    none
TEXT OUTPUT            path plus matching line numbers; no snippets
LINKED WORKTREES       selected member only
HOSTEXPLICIT           exactly 11; repo.search ineligible
PROFILE                explicit profile_version 1 capability
PERSISTENCE            none
NETWORK                none
MUTATION               none
NEW ADR                none
NEW DEPENDENCY         none expected
NEXT TASK              Task 367 — production implementation
```

**少一點流程摩擦，不少任何安全與正確性證據。**
