# Task 484 — Bounded untracked-file correction Tool implementation

## Starting checkpoint

Dedicated worktree: `F:\coding\otherPrj\rah-task-484`. After `git fetch origin`, `HEAD` and `origin/master` both equaled `105e79198edbde250a33990cf23882dd58d35e05`; `git status --short` was empty. The dirty primary checkout is outside this task.

## Contract and naming

Chosen name: `repo.edit-untracked-file`. This distinguishes its one-existing-untracked-file authority from HEAD-tracked `repo.patch` and `repo.edit-files`, and create-only `repo.create-file`.

The request uses the established one-file patch field names: `path`, `expected_file_sha256`, `expected_file_byte_length`, and exact literal replacement fields. The SHA-256 and byte length cover the complete current content, and the old text must occur exactly once. The result follows the one-file mutation status, `changed`, `uncertain`, and redacted `reason` convention.

## Source and architecture analysis

The review used Task 483's scope report, README, architecture and security documentation, and ADRs 0012, 0013, 0014, 0027, and 0029. The existing one-file patch engine already owns path validation, root and file-object identity, nested-repository rejection, bounded UTF-8/literal transformation, shared repository lease, same-parent exclusive temporary, native replacement, and conservative uncertainty. The new Tool has a separate private host policy that fixes an untracked target class while reusing those mechanics. ADR 0031 records the new authority without changing accepted tracked or create-only contracts.

## Request, result, and state

The closed request accepts `path`, `expected_file_sha256`, `expected_file_byte_length`, and either the existing one-pair `expected_old_text`/`replacement_text` form or one through sixteen `replacements` pairs. The digest and byte length cover exact raw preimage content; every old text match must be unique in the original strict-UTF-8 snapshot. No fuzzy match or patch language is added. A verified all-equal replacement is a no-op.

The result reports `status`, `changed`, `uncertain`, redacted `reason`, and the logical `path`. Normal success is `ok/true/false/none`; verified no-op is `ok/false/false/none`. Pre-commit refusal is `precondition_failed/false/false`; reason distinguishes stale text, repository state, ignored target, missing target, non-regular target, unsupported text, and path/filesystem rejection. A proven no-effect replacement failure is `replacement_failed_known`; lost post-observation is `uncertain` and is not replayed.

Positive Git state is one existing regular file under the active admitted root that is absent from current HEAD and all index stages (including intent-to-add), is not ignored by Git's current rules, and is reported as exactly one literal `ls-files --others --exclude-standard` path. This excludes staged adds, current-HEAD entries deleted from index, tracked files, ignored files, nested repositories, gitlinks, links/reparse points, hard-link aliases, and case aliases. The selected linked worktree's own root, HEAD, index, and identity are used. No other branch grants or blocks authority by itself.

The mutation captures bounded preimage and identity evidence under the shared lease, prepares and flushes one same-parent temporary, revalidates file, path, repository and Git state, and makes at most one native replacement attempt. The untracked policy repeats the final revalidation at the immediate replacement boundary; a staged transition there is proved to refuse with zero attempt. Post-observation verifies exact content and untracked state. The Tool never stages, commits, changes refs, or selects a repository. HostExplicit remains the exact 11-name allowlist.

## Changed surfaces

- `crates/rah-tools/src/repository_worktree_patch.rs`: distinct untracked Tool/private policy using the existing one-file replacement engine and Git-state checks.
- `crates/rah-tools/src/trusted_profile.rs` and `src/lib.rs`: closed symbolic binding and export.
- `crates/rah-profile-composition`: ordinary ToolRegistry construction and production-path acceptance test.
- `crates/rah-desktop/src/effective_authority.rs`: ordinary repository mutation metadata; `src/host_invocation.rs`: exact 11-name HostExplicit regression assertion.
- `crates/rah-tools/tests/repository_untracked_file_edit.rs`: product and negative acceptance matrix.
- README, architecture/security docs, ADR 0031, and this task report: permanent contract and evidence.

## Validation evidence

Focused: one-file mutation suite 54 passed before the final identity-race case; repository-boundary suite 19 passed; create-file integration 11 passed; profile composition 5 passed after building its required MCP/Plugin echo helpers; Desktop HostExplicit allowlist 1 passed. The final untracked integration suite passed 8 cases on Windows, including a Windows case alias; the final untracked-filtered unit run passed 7 cases, including state-transition, identity-race, native-failure, and uncertainty tests. Relevant Node frontend static tests passed 2/2.

The final exact-state `cargo test --workspace` pass reported 1,010 passed, zero failed, 24 intentionally ignored across 55 result suites. `cargo fmt --check`, `cargo check --workspace`, and `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed on that state. The first Clippy run had found one `collapsible_if` style lint in the new revalidation branch; it was corrected before the final pass. `git diff --check` passed.

Final exact-state canonical Windows Desktop helper-prepared gate: PASS, helper exit 0, Desktop test exit 0, 324 passed, zero failed, 20 intentionally ignored, no watchdog timeout. Evidence: `F:\Temp\rah-task484-desktop-gate\20260930-211931-560-e5660363c8a6474894942793975722be\status.json`.

After the final deterministic and Windows gates, the fresh composed Tool acceptance test created a disposable committed repository and untracked `docs/local-setup.txt` with `worker_count=44`, dispatched the Tool through `authorized_tool_dispatch` and the trusted-profile `ToolRegistry`, and verified `worker_count=4`, unchanged untracked status, index bytes, HEAD, unrelated committed content, and only the expected file in `docs`. This is the normal production Tool path; each test execution allocates a new fixture repository. The post-gate invocation passed 1/1.

**A — v0.33 UNTRACKED-FILE CORRECTION IMPLEMENTED AND VALIDATED.** All required deterministic, Windows, live, authority, and no-collateral-effect checks passed. The implementation is eligible for a scoped commit and normal push. The self-referential commit SHA and subsequent push/CI observations are recorded in the final Task 484 return; they cannot be embedded in this same commit's file bytes. No version bump, tag, or release is included.

## Exact-head CI follow-up

The initial implementation commit `59ba543290c899e2aeb684503a8439a5ca4d4dde` reached both configured push destinations. Its GitHub push CI run 36721755504 failed in the Linux `rah-tools` unit suite: the newly added identity-race test removed and recreated its target, allowing the filesystem to reuse the just-freed inode. That simulation did not prove an identity change on Linux. The follow-up test correction retains the original file under a separate name while creating the same-byte replacement, forcing two live file objects before revalidation. This is a test-fixture correction; the Tool implementation is unchanged. Task 484 remains open until the follow-up exact-head CI passes.
