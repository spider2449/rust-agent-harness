# RAH v0.32.0 Release Gate

Status: **IN PROGRESS — REMAINING LOCAL GATES PASS; COMMIT, PUSH, AND EXACT-HEAD CI PENDING**

This gate keeps the milestone scope, implementation, deterministic evidence,
independent review, live Windows evidence, release preparation, and future
publication as distinct records. No tag or GitHub Release is created by Task
412.

## Milestone scope

Task 374 selected **Bounded Repository Structure Listing / Browse** as the sole
v0.32 primary capability. The frozen capability is `repo.list`, a bounded
direct-child structural listing for the host-selected active repository.
HostExplicit remains exactly 11; `repo.list` is not HostExplicit.

## Contract and implementation

| Task | Evidence |
| --- | --- |
| 374 | v0.32 scope and authority roadmap; sole capability and initial non-goals. |
| 375 | Closed `repo.list` contract: `{}` lists root; optional validated relative `path`; direct children only; fixed tracked inventory; synthesized directories; bounded deterministic projection; sanitized closed errors. |
| 376 | Initial production implementation of the frozen contract, including profile composition and active Desktop registry registration. |

`repo.list` uses the fixed tracked Git inventory and host-side projection. It
returns repository-relative structural metadata (`path`, `kind`) and does not
read file contents. The host owns the selected active repository and
projection boundary. Effective authority remains `ReadOnly` /
`RepositoryObservation` / `Execute` / `repository_bound=true`.

## Independent audit and correction history

| Task | Recorded result |
| --- | --- |
| 377 | Independent audit and hardening; accepted with the recorded narrow correction. |
| 379 | Initial Windows live certification failed. Preserve this failed intermediate result. |
| 380 | First correction to the Windows/boundary path. |
| 381 | Independent boundary re-audit found a remaining symlink-ancestor flaw. Preserve this finding. |
| 381-A | Hardened existing-ancestor classification and boundary validation. |
| 381-B | Independent verification: **PASS**. |
| 382 | Windows recertification: **PASS WITH EXPLICIT NONCLAIMS**; Windows symlink live and Codex inference certification were not established. |

The correction history is not collapsed into a claim that every intermediate
attempt passed. The accepted implementation and its boundary are those
verified after the Task 381-A correction and Task 381-B audit.

## Deterministic binary evidence

Task 408 found that the explicit tracked-binary no-content evidence expected
by Task 374 was missing. It was an evidence gap, not a reported production
defect.

Task 409 added deterministic evidence and committed it as:

```text
580880f2619de4ed1e3810aec31b2ad27d6d91c3
test: cover repo.list binary no-content contract
```

The regression proves a tracked binary file can be listed structurally without
disclosing its contents, sentinel, or byte sequence. This is evidence for
structural non-disclosure in the tested contract; it does not claim general
arbitrary-byte semantics.

## Task 410 validation-failure disposition

Task 409 encountered four Desktop test failures before certification. Task 410
could not reproduce them: all four focused tests passed 3/3, and two
consecutive full serial Desktop suites passed at 320 passed, 0 failed, 18
ignored, 338 discovered. The Desktop source tree was unchanged from the prior
known-pass checkpoint. The original failures remain recorded; they were
dispositioned as transient and non-reproduced, not labeled fixed or erased.

## Windows live binary evidence

The immutable Task 411 certification source HEAD is:

```text
64467c9d8dfbb0cb465a8be72e3c312bc34c876a
test: add repo.list binary Windows certification case
```

That exact committed HEAD was live-certified with a clean worktree. The later
documentation closeout is a separate HEAD:

```text
ad9e755ac6d973fb39c58ad88e905c682e6fe9ba
docs: record repo.list binary Windows certification
```

Do not describe the documentation HEAD as the certification source. Task 411
verdict:

```text
PASS — REPO.LIST BINARY-FILE WINDOWS LIVE EVIDENCE CLOSED
```

The certification environment was:

```text
Microsoft Windows 11 IoT Enterprise LTSC
10.0.26100; build 26100; UBR 9457; 24H2; x64
rustc 1.98.1 (48a229cea 2026-09-01)
Cargo 1.98.1 (797e8a9bc 2026-08-05)
Git 2.55.0.windows.5
```

`codex-cli 0.156.1` was observed but unused. No model inference or credentials
were used.

The committed harness verified that `binary-sentinel.dat` was Git-tracked and
that the live active-Desktop-registry request `{}` returned:

```json
{"kind":"file","path":"binary-sentinel.dat"}
```

The response had no content field; the sentinel and raw host path were absent.
Active-repository isolation passed. Main/A/B state mutation checks passed.
HostExplicit remained exactly 11, and `host_kind("repo.list") == None`.
The binary payload is intentionally not reproduced here. The evidence is
structural non-disclosure for this fixture, not general arbitrary-byte
certification.

## Explicit nonclaims

- No arbitrary filesystem enumeration.
- No untracked or ignored discovery.
- No recursive browse.
- No content-reading capability.
- No mutation authority.
- No HostExplicit expansion.
- No multiple-active repository support.
- No model-selected repository.
- No general Windows symlink certification.
- No Codex inference certification.
- No arbitrary-byte semantics beyond structural non-disclosure.
- No transactional snapshot, race-free TOCTOU, rollback, or replay guarantee.

These are boundaries, not roadmap promises.

## Unrelated known issue

The existing Tauri permission-generation discrepancy remains unrelated debt:
47 invoke handlers versus 44 generated permission/capability entries. The
known missing representations are `host_prepare_repo_edit_files`,
`host_prepare_repo_create_file`, and `host_prepare_repo_delete_file`. Task 408
found it outside the `repo.list` ToolRegistry/Generic Tool Bridge route and
not a v0.32 blocker. Task 412 does not fix it.

## Release preparation

Task 412 updates the RAH workspace version from `0.31.0` to `0.32.0`, refreshes
only internal Cargo lockfile metadata, and updates release documentation. It
does not change Rust source or `repo.list` behavior. Its first serial workspace
validation failed in two Desktop repository-observation tests and did not emit
a final test summary. This failed result remains historical evidence and is
not rewritten as a pass. See
[`docs/plans/2026-09-25-task-412-v0.32-release-preparation.md`](plans/2026-09-25-task-412-v0.32-release-preparation.md)
for the exact initial validation and resumption history.

### Release validation stop and correction chain

| Task | Evidence and disposition |
| --- | --- |
| 412 | Initial release-preparation workspace validation reported intermittent Desktop failures in `observed_single_target_stage_and_unstage_refresh_and_consume_selectors` and `old_repository_selector_cannot_resolve_to_new_repository_action`; no final summary was emitted. |
| 413 | Classified the workspace failures as intermittent; preserved the original failure and bounded validation follow-up. |
| 414 | Isolated shared staged-observation timeout exhaustion in the serial Desktop test family. |
| 415 | Stopped implementation because the staged timeout policy required clarification. |
| 416 | Clarified the canonical policy: one aggregate 15-second `IndexVsHead` staged-observation deadline. |
| 417 | Enforced the shared deadline through fixed Git-layout probes; package validation still exposed timeout behavior. |
| 418 | Healthy timing did not justify increasing the 15-second bound. |
| 419 | Isolated the HEAD child-timeout composition defect. |
| 420 | Corrected HEAD child timeout to `min(FILE_INFO_TIMEOUT, aggregate remaining)`. |
| 421 | Dispositioned a linked file-info package failure as non-reproducible; its original cause was not identified. |
| 422 | Independently verified the combined correction, deterministic timeout coverage, package/workspace checks, and preserved authority/identity invariants. |
| 423 | Resumes release preparation, including fresh production-code Windows `repo.list` recertification and final release validation. |

The correction is committed at `83c5c30446a2f566be7b660d45e6d70b98a8d344`
(`fix: enforce staged observation timeout composition`). It preserves the
15-second aggregate staged deadline, clamps Git-layout probes to aggregate
remaining time, and clamps HEAD child execution to
`min(FILE_INFO_TIMEOUT, aggregate remaining)`. Timeout exhaustion remains
fail-closed. Frozen values remain `DIFF_TIMEOUT = 15s`,
`FILE_INFO_TIMEOUT = 5s`, and `PROBE_TIMEOUT = 5s`: no timeout was increased,
validation was reduced, retries were added, or original intermittent cause
identified.

The original Task 412 fingerprint before authorized release-record updates
was `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.

### Fresh production-code Windows certification

Task 423 ran the established Task 382/411 Windows live harness from a clean
temporary worktree at the exact correction commit
`83c5c30446a2f566be7b660d45e6d70b98a8d344`. Result: **PASS**, 1 passed, 0
failed, 337 filtered out; the binary tracked-entry, closed schema/no-content,
sentinel absence, raw path absence, active-repository isolation, and standard
repo.list matrix passed. Environment: Windows 10 IoT Enterprise LTSC 2024,
build 26100, x64; rustc 1.98.1; Cargo 1.98.1; Git 2.55.0.windows.5. The
existing Windows symlink live nonclaim remains. This live result is tied to the
correction commit; release-preparation changes may rely on it only while
subsequent changes remain package versions, internal lockfile package
versions, and release documentation/records.

### Resumed release validation

Task 423 must record final metadata, dependency/features/workspace drift audit,
two consecutive serial workspace test runs, formatting, workspace check,
Clippy, diff checks, and every applicable Task 370 release check below. The
original Task 412 failure remains separate from these resumed results.

| Gate | Task 423 result |
| --- | --- |
| Metadata: 13 workspace packages/members, all `0.32.0`, edition `2024` | PASS; no membership change observed |
| External dependency/checksum drift; feature drift | PASS; no manifest or external lockfile changes |
| `repo.list` fresh Windows certification at correction commit | PASS; exact checkpoint and environment recorded above |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace -- --test-threads=1` run 1 | PASS; 990 passed, 0 failed, 22 ignored across test binaries; aggregate binary duration 2083.95 s |
| `cargo test --workspace -- --test-threads=1` run 2 | PASS; 990 passed, 0 failed, 22 ignored across test binaries; aggregate binary duration 2094.69 s |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test -p rah-tools -- --test-threads=1` | **FAIL**; 345 passed, 1 failed, 0 ignored in 998.59 s. `repository_commit::tests::linked_commit_uses_only_the_selected_worktree_index_and_branch` panicked at `repository_commit.rs:1131:74` because Git layout identity validation returned an execution error. Cause is not identified. Full output is preserved in the Task 423 run log. |
| Other Task 370 package checks | Not run after the required `rah-tools` check failed. |
| `cargo metadata --no-deps --format-version 1`; `git diff --check` | PASS before release validation; metadata confirmed above and Rust diff was empty. |
| Release-preparation commit, post-commit validation, push, exact-head CI | Not started; stopped at failed applicable package gate. |

```text
release preparation: STOP — ESTABLISHED `rah-tools` RELEASE CHECK FAILED
tag: NOT CREATED
GitHub Release: NOT CREATED
publication: NOT STARTED
```

At the end of Task 423, no cause or disposition for its standalone `rah-tools`
failure was established. It was not inferred to share a cause with the Task
412/Desktop failures or Task 421's linked file-info case. Task 423A's later
bounded disposition and Task 423B's continuation are recorded below; the
original failure remains preserved.

### Task 423A disposition and Task 423B continuation

Task 423A dispositioned the standalone `rah-tools` failure as:

```text
PASS — LINKED COMMIT GIT-LAYOUT FAILURE DISPOSITIONED AS NON-REPRODUCIBLE
```

It preserved the original failure as historical evidence. The exact failing
Git-layout probe and root cause remain unknown; the bounded PASS does not prove
that the original failure is impossible. Task 423B verified its starting HEAD
as `bd6eb58419860e23f004d0341f953399f8466b88`, equal to `origin/master`, with
the Task 423A disposition and timeout correction commits present at/under
HEAD. The pre-existing Task 423 tracked release diff matched fingerprint
`66c7d9b0bfc2e27fb66ae79dc041968a4f3e4b67`; no staged or Rust/test source
changes were present.

Task 423B reused Task 423's metadata, dependency/checksum/feature/workspace
audit, fresh correction-commit Windows `repo.list` certification, formatter,
workspace check, workspace Clippy, diff check, and two serial workspace PASS
runs (990 passed, 0 failed, 22 ignored each). It also reused Task 423A's
focused and package disposition matrices (exact test 5/5, linked family 3/3,
`repository_commit` family 3/3, `rah-tools --lib` 3/3, full package 3/3).
These results remain applicable because Task 423B made no Rust/test source
changes. None of those expensive matrices or Windows live tests was rerun.

Following the documented Task 423 order, Task 423B ran the remaining
Task 370 package gates serially:

| Gate | Task 423B result |
| --- | --- |
| `cargo test -p rah-profile-composition -- --test-threads=1` | PASS; 4 passed, 0 failed, 0 ignored; command 48.25 s, test binary 4.56 s |
| `cargo test -p rah-desktop -- --test-threads=1` | PASS; 320 passed, 0 failed, 18 ignored; command 1109.26 s, test binary 956.19 s |
| `cargo test -p rah-runtime-codex -- --test-threads=1` | PASS; 102 passed, 0 failed, 1 ignored across three binaries; command 509.73 s |
| `cargo metadata --no-deps --format-version 1` | PASS; 13 packages/members, all `0.32.0`, edition 2024 |
| `git diff --check` | PASS |

Final audit found only the expected 13 internal lockfile version updates and
workspace version change relative to the correction checkpoint. External
dependency versions/checksums, features, and workspace membership are
unchanged. No Rust/test source or authority implementation changed;
`repo.list` remains read-only `RepositoryObservation`, HostExplicit remains
exactly 11, and the unrelated Tauri permission-generation debt remains out of
scope. Release-preparation commit, push, and exact-head CI are pending; no
publication has begun.

## Publication state

No `v0.32.0` tag, tag push, GitHub Release, or release artifact publication
has been performed. Task 424 remains reserved for publication and may start
only after Task 423B has a validated release-preparation commit pushed to
`origin/master` with passing CI for that exact commit SHA.
