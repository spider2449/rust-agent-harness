# Task 465 — Repository root validation rejection audit

## Checkpoint and scope

Started from published `ca51c8896b889fcaa413ea22463c1277d4ebf0ec` in clean `F:\coding\otherPrj\rah-task-465`. Task 462/463 WIP in `F:\coding\otherPrj\rah-task-457` was not touched. Task 464 observed the production picker reject a 221-character ordinary directory, then the same directory after `git init`, both with “Selected folder is not a valid repository root.” The location never reached the editor. Task 462 clipping acceptance remains unverified; no clipping defect follows from this observation.

## Exact selection and rejection path

The click handler in `crates/rah-desktop/frontend/status.js` invokes `choose_repository` and renders its returned error through `errorMessage` (near lines 2343–2355). `errorMessage` maps `repository_invalid` to the quoted text (line 161). The host command `choose_repository` in `crates/rah-desktop/src/main.rs` (near lines 7616–7647) checks selection and connection state, calls `blocking_pick_folder`, converts the picker result with `into_path`, discovers Git, and calls `admit_repository`. On production Windows, `admit_repository_with_semantic_validation` (near lines 6064–6095) captures `RepositoryAdmissionIdentity`, validates Git semantics, constructs the Desktop repository and authorities, revalidates, then admits the member. Capture, semantic validation, and mandatory construction failures can all become `FrontendError::RepositoryInvalid`; the underlying `ToolError` is deliberately not returned to the frontend. Picker conversion instead returns `RepositoryDialogFailed`; Git discovery returns `GitUnavailable`.

`RepositoryAdmissionIdentity::capture` in `crates/rah-tools/src/repository_admission_identity.rs` (near lines 107–127) canonicalizes the selected directory and Git executable and captures `RepositoryGitLayout`. `RepositoryGitLayout::capture` in `repository_git_layout.rs` (near lines 177–210) requires the selected root itself to contain supported `.git` metadata. `validate_git_with_budget` (near lines 358–450) runs bounded fixed Git probes for toplevel, private/common Git directories, non-bare status, superproject, index and HEAD paths, and `git worktree list --porcelain -z`, comparing canonical paths and the registered selected worktree. `worktree_record_matches` (near lines 740–790) requires the selected record's `HEAD` field to be a 40- or 64-digit hex object ID and an attached branch or detached marker. An unborn worktree record fails this condition. This is a resolvable HEAD requirement, even though the validator does not run `git rev-parse --verify HEAD` itself.

## Root contract

| Condition | Admission requirement evidenced by implementation |
| --- | --- |
| `.git` presence | Yes, at the selected root; a real directory or validated linked-worktree gitfile. |
| `--show-toplevel` | Must succeed and canonicalize to the selected root. A subdirectory is not admitted as its own root. |
| Non-bare / normal worktree | Required; selected worktree must be registered, non-bare and non-prunable. Main and supported registered linked worktrees are recognized. |
| Valid HEAD / commit | Required by the worktree-list record's hex HEAD object ID. Thus plain `git init` without a commit is rejected. Detached HEAD can satisfy this specific layout check; narrower capabilities may have stricter rules. |
| Index | Git's reported index path must match the captured layout. An existing index file or staged entry is not required by this admission check. |
| Linked administrative path | Not a selected worktree root; linked worktree gitfile/backlink relationships are checked separately. |

This is an intentional current contract in code: the worktree-list parser explicitly rejects an absent or invalid HEAD OID. The audit found no authoritative promise that an unborn repository must be selectable. The fact that some individual capabilities document unborn support does not override Desktop's admission contract.

## Controlled fixtures and evidence

Disposable fixtures were created outside source repositories under `F:\temp\rah-task465-fixtures-20260928`, containing only a harmless text file where needed. Long roots were 208–211 characters. Git was `C:\Program Files\Git\cmd\git.exe`. Long fixture Git repositories used local `core.longpaths=true`: without it, Git initialized the 211-character root but `git add`/`commit` failed while writing `.git/objects` with “Filename too long.” That was a fixture creation prerequisite, not a RAH validation result.

| Fixture | Root length | Git `--show-toplevel` / `--git-dir` / `--is-bare-repository` | `--verify HEAD` | `status --porcelain=v1` | RAH identity capture + semantic validation |
| --- | ---: | --- | --- | --- | --- |
| A short ordinary directory | 48 | 128 / 128 / 128 | 128 | 128 | Rejected at layout capture |
| B short unborn Git | 46 | 0 / 0 (`.git`) / 0 (`false`) | 128 | 0 empty | Rejected at semantic validation |
| C short committed Git | 49 | 0 / 0 (`.git`) / 0 (`false`) | 0 commit OID | 0 empty | Accepted |
| D long unborn Git | 208 | 0 / 0 (`.git`) / 0 (`false`) | 128 | 0 empty | Rejected at semantic validation |
| E long committed Git | 211 | 0 / 0 (`.git`) / 0 (`false`) | 0 commit OID | 0 empty | Accepted |

The RAH result came from one temporary `rah-tools` unit probe calling the existing `RepositoryAdmissionIdentity::capture` and `validate_git` against A–E. `cargo test -p rah-tools task465_disposable_fixture_probe -- --nocapture` passed (one test, 348 filtered); the temporary probe was then removed. No production code or test change remains. The same validator accepted C and E, so the Task 464 unborn result does not implicate long-path handling. The production picker was not repeated: the deterministic comparison resolves the fixture distinction. The exact Task 464 picker-returned representation was not captured, so this audit does not certify that path form for a later committed selection.

## Path and message findings

The host passes the picker `into_path` result to admission without frontend path serialization. Admission canonicalizes the directory; Git-reported paths are canonicalized before comparison, and Windows object identity is captured for the root and Git metadata. The controlled long committed path passed this comparison. Textual drive-path versus extended-length-path differences were therefore not treated as different directories. No production picker path-form defect was proven.

The frontend's single `repository_invalid` sentence covers at least a non-repository directory, an unborn repository, semantic layout mismatch, and mandatory repository construction failures. It does not identify the actual condition, but it does accurately say that the selected folder fails the current repository-root admission contract. Distinct errors exist for picker conversion and unavailable Git. A later UX task could provide a more specific explanation for unborn repositories; this audit does not establish that wording as the primary defect.

## Classification and next task

**A — Task 464 used an invalid fixture; a normal committed repository is required.** Both short and long committed fixtures passed the exact RAH identity and semantic validation used by production admission. Both unborn fixtures failed the same semantic stage, consistent with the explicit worktree-list HEAD rule. No repository-validation fix is justified by Task 464. Task 462 production/test WIP remains frozen and its long-path, narrow-window, and Task 460 chooser live acceptances remain unverified.

The next task should resume Task 462 acceptance with a long normally committed Git repository, including any Git long-path setting required to create its commit, then reconcile this documentation-only Task 465 commit with Task 462's older-base WIP explicitly before publishing that WIP. Do not rebase Task 462 automatically. RAH remains 0.32.0, HostExplicit exactly 11, certified Codex 0.157.1, and v0.33 capability none selected. No tag or release.
