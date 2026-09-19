# RAH v0.31.0 Release Gate

Status: **READY FOR PUBLICATION - NOT YET RELEASED**

Task 370 is the v0.31 milestone audit and release preparation. This gate stops
before creation of the annotated tag, tag push, GitHub Release, and any
post-release cleanup.

## Release theme

**Bounded Repository Discovery / Search**

RAH adds one canonical first-party Tool, `repo.search`, with closed `path` and
`text` modes. It locates tracked files and bounded literal matches inside the
host-selected active repository worktree so existing read and authoring Tools
can be used after discovery.

## Milestone audit

| Task | Accepted result |
| --- | --- |
| 365 | v0.31 scope and authority roadmap; Bounded Repository Discovery / Search selected as a **NARROW EXTENSION**. |
| 366 | Closed `repo.search` contract: `path` / `text`, tracked files, selected active worktree, fixed Git inventory, host-side literal matching, bounded best-effort observation. |
| 367 | Production implementation of the frozen contract. |
| 368 | Independent audit: **PASS WITH NARROW HARDENING**; `non_regular` omission accounting and linked-worktree path coverage retained. |
| 369 | Windows live certification: **PASS WITH EXPLICIT CODEX NONCLAIM**; one narrow Desktop registration omission was corrected in the certified candidate. |
| 370 | This milestone audit and release preparation; no product capability or authority change. |

No mandatory v0.31 release blocker remains. The Tasks 365-369 chain is
accepted as one coherent milestone, with the explicit nonclaims below retained
as nonclaims rather than converted into unsupported claims.

## Source identities

| Source | Identity |
| --- | --- |
| Task 367/368 implementation checkpoint | `152a5fff96f621083a3ec43892ecdeffc2f6ff62` (`feat: add bounded repository search`) |
| Task 369 complete Windows-certified production candidate | `c5a67f10b334299cc5a5de49c37e94eb38640c5a` (`fix: complete repository search desktop integration`) |
| Task 370 release-preparation source | this commit; exact SHA is recorded after commit |

`152a5ff` is not the complete production source: Task 369 corrected the narrow
Desktop registration omission and certified `c5a67f1` as the complete
production candidate.

## Product and authority boundary

`repo.search` is precisely:

- tracked-file repository discovery in the selected active worktree only;
- `path` mode and `text` mode;
- literal, case-sensitive host-side matching;
- bounded output and best-effort observation;
- text matching against current selected-worktree bytes.

The fixed inventory source is:

```text
git --no-pager ls-files --cached --deduplicate -z --full-name --
```

Git supplies tracked inventory only. RAH does not use `grep`, `ripgrep`,
`git grep`, a filesystem search shortcut, regex, fuzzy or semantic matching,
or a persistent/background index.

The verified effective authority is:

```text
EffectClass       = ReadOnly
AuthorityCategory = RepositoryObservation
PermissionLevel   = Execute
repository_bound  = true
```

The exact HostExplicit set remains exactly these 11 names:

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

`repo.search` is not HostExplicit-eligible. `repo.create-directory` remains
not HostExplicit-eligible. There is no category, prefix, permission-derived,
provider-derived, or generic Git admission rule.

## Linked-worktree isolation

The active member remains host-selected and there is one active-only
ToolRegistry. The identity boundary remains:

```text
same common Git directory != same repository member
shared common Git state != sibling executable authority
```

Active linked A may search A's tracked worktree files. It cannot search main or
linked B contents. No sibling selector and no union ToolRegistry exists.

## Privacy and confinement

Search is tracked-only, selected-active-worktree-only, and rejects nested
repository boundaries and symlink, junction, or Windows reparse traversal.
Requests, inventory, file reads, matching lines, normalized output, and time
are bounded. Model-visible output is limited to safe logical repository data:
repository-relative UTF-8 paths, matching line numbers, bounded omission
counts, and completion/truncation state.

The following remain private and are not exposed through Tool output,
authority serialization, activity, errors, or provider state: absolute root,
private Git directory, common Git directory, worktrees registration,
gitfile/backlink/commondir identity evidence, filesystem identities, Git
executable path, profile source path, raw stderr, repository generations, and
ToolRegistry authority handles.

## Certification environment and verdict

Task 369 certification environment:

| Item | Certification-time value |
| --- | --- |
| Windows | Windows 11 IoT Enterprise LTSC, build 26100, x64 |
| rustc | 1.98.1 |
| Cargo | 1.98.1 |
| Git | 2.55.0.windows.5 |
| Workspace | 13 packages, `0.30.0` at certification time |
| Rust edition | 2024 |
| Codex baseline | `codex-cli 0.149.0` unavailable; ambient `0.155.1` not substituted |

Task 370 release-preparation metadata is distinct from certification-time
metadata:

```text
13 workspace packages
0.31.0
edition 2024
```

Certification verdict: **PASS WITH EXPLICIT CODEX NONCLAIM**.

Passed evidence includes path mode, text mode, tracked-only scope, current
worktree bytes, untracked/ignored exclusion, binary/oversized omission,
linked-worktree isolation, active switching, nested repository rejection,
junction/reparse rejection, result saturation, privacy, Effective Authority,
HostExplicit exactly 11, Generic Tool Bridge dispatch, and no intentional
repository mutation.

### Codex 0.149.0 release decision

The exact project lifecycle binary `codex-cli 0.149.0` was unavailable on the
Task 369 host. Ambient `0.155.1` was correctly not substituted. The real
model-selected Codex inference path under `0.149.0` was therefore not
certified.

This is an **EXPLICIT RELEASE NONCLAIM, NOT A v0.31 RELEASE BLOCKER**. The
production `repo.search` behavior and actual Generic Tool Bridge dispatch were
Windows-certified, and no evidence indicates a search authority or correctness
defect. The gate does not claim model-selected inference certification.

### Windows symlink decision

Windows symlink live behavior was unavailable on the Task 369 host. Deterministic
coverage exists and junction/reparse live behavior passed. Symlink live behavior
is an **EXPLICIT NONCLAIM, NOT A RELEASE BLOCKER**. This gate does not state
that Windows symlink live behavior passed.

## Parallel-test contention

Task 368 established that the observed parallel repository-test contention
family reproduces against the clean pre-Task-367 baseline. It is not evidence
of a v0.31 `repo.search` regression. The deterministic serial workspace suite
is the local release gate where necessary:

```text
cargo test --workspace -- --test-threads=1
```

The default parallel workspace suite is not represented as passing by this
gate. A future parallel failure in the already-proven family must remain
separately recorded; unrelated tests must not be modified to manufacture a
green run. Exact-head CI is an independent signal.

## Release nonclaims

This gate does not claim:

- Linux live certification, macOS live certification, or cross-platform live parity;
- real `codex-cli 0.149.0` model-selected `repo.search` inference certification;
- Windows symlink live certification or GUI automation certification;
- untracked search, ignored-file search, regex/fuzzy/semantic search, or a persistent/background index;
- sibling-worktree search, multiple active repositories, or Git worktree lifecycle authority;
- generic Git authority, generic shell/process authority, or generic filesystem write authority;
- an OS sandbox, network isolation, race-free TOCTOU, or a machine-wide external Git lock;
- rollback, compensation, or replay after uncertain effects;
- a transactional repository snapshot or stronger effect guarantee than best-effort observation.

Existing RAH nonclaims remain in force: process supervision is not OS
sandboxing; no network isolation, rollback, compensation, or replay is implied;
uncertain external effects are not automatically replayed.

## Version and dependency gate

Task 370 changes the 13 RAH workspace packages from `0.30.0` to `0.31.0` and
keeps Rust edition `2024`. `Cargo.lock` changes only the 13 internal RAH
package version records. External dependency versions, checksums, sources,
and the dependency graph must remain unchanged.

## Task 370 validation record

The required local release gates passed:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace -- --test-threads=1` | PASS; no failures across the workspace; Desktop 314 passed / 17 ignored in the workspace run |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS |
| `cargo metadata --no-deps --format-version 1` | PASS; 13 packages, all `0.31.0`, edition 2024 |
| `cargo test -p rah-tools -- --test-threads=1` | PASS; 326 unit tests and all integration suites; all 7 repository-search tests passed |
| `cargo test -p rah-profile-composition` | PASS; 4 tests |
| `cargo test -p rah-desktop -- --test-threads=1` | PASS; 314 passed, 17 ignored |
| `cargo test -p rah-runtime-codex -- --test-threads=1` | PASS; 84 passed, 1 ignored; Generic Tool Bridge search dispatch passed |
| external dependency drift check | NONE; Cargo.lock changed only 13 internal RAH version records |

The required commands were:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
cargo test -p rah-tools -- --test-threads=1
cargo test -p rah-profile-composition
cargo test -p rah-desktop -- --test-threads=1
cargo test -p rah-runtime-codex -- --test-threads=1
```

The default-parallel `cargo test --workspace` signal was not rerun for Task
370. Task 368's clean-baseline comparison remains the evidence for the known
parallel contention family; it is not represented as a passing local result.
The repository CI workflow's exact-head result remains an independent release
signal.

## Publication hard stop

Task 370 ends with the release-preparation commit, a clean worktree, and this
gate status. It does not create `v0.31.0`, push a tag, create a GitHub Release,
mark the changelog released, or perform post-release cleanup. Publication is a
separate authorized task.
