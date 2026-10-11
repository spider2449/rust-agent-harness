# Task 416 — Staged-Diff Timeout Policy Design

Date: 2026-09-25
Status: **PASS — STAGED-DIFF TIMEOUT POLICY CLARIFIED**
Task type: documentation and design research only

## Verdict

```text
PASS — STAGED-DIFF TIMEOUT POLICY CLARIFIED
```

The canonical policy is one aggregate 15-second deadline for the staged-diff
observation sequence. The budget starts immediately before the pre-HEAD
observation and covers every repository-layout validation, every fixed Git
child in the staged sequence, and the pre/post HEAD identity observations. It
does not refresh between observer phases. The 15-second limit is a deadline
for the bounded Git observation phase; bounded in-process parsing and output
normalization follow the final observation under existing output/count caps.

This policy is supported by the original observer design record, not by the
constant name alone. Task 061 described `repo.diff` as having an “aggregate
15-second timeout”; Task 058's observer policy table called both `repo.diff`
and `repo.diff-staged` “15 s total”; Task 062 reused Task 061's limits and
implemented staged observation with one `Instant` passed across all five
observer phases. No source supports refreshing the deadline between phases.

Current code does not strictly enforce the selected aggregate deadline during
layout validation: each validation probe is individually capped at five
seconds, but `RepositoryGitLayout::validate_git` receives no remaining-time
budget. The elapsed-time check occurs after validation. Task 417 must enforce
the selected aggregate policy across validation probes while preserving the
15-second total and all identity checks. This is a narrow enforcement
correction, not a policy extension.

## Starting checkpoint and preserved release state

Starting committed HEAD:

```text
db0d99c2bdda3f5264ad71f960105da5de40cf6d
docs: isolate intermittent desktop git observation failures
```

Starting dirty paths were exactly the frozen Task 412 paths plus the Task 415
record:

```text
CHANGELOG.md
Cargo.lock
Cargo.toml
README.md
docs/ARCHITECTURE.md
docs/SECURITY.md
docs/RAH_V0.32_RELEASE_GATE.md
docs/plans/2026-09-25-task-412-v0.32-release-preparation.md
docs/plans/2026-09-25-task-415-staged-diff-observation-timeout-correction.md
```

The Task 412 eight-file binary diff fingerprint was verified before research:

```text
ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3
```

Task 414's record documents intermittent exhaustion of this shared budget in
the serial Desktop package suite. Task 415 stopped before changing code
because resetting the timer after validation would silently change the
aggregate policy. Neither Task 412 validation nor Task 417 implementation was
resumed here.

## Timeout history and original contract

| Evidence | Finding | Classification |
| --- | --- | --- |
| Task 058 research, `docs/RAH_V0.6_REPOSITORY_OBSERVER_RESEARCH.md` | The policy table specifies `repo.diff` and `repo.diff-staged` at 15 seconds total; any timeout rejects the observation with no partial output. | Historical design contract |
| Task 061, commit `e1c0adf1d802f10b1e99963c99280e93b6c7f8b2`, “feat: add deterministic repository diff observer” | Introduced `DIFF_TIMEOUT = 15s` for `repo.diff`, a single start instant shared by its three raw/numstat/patch commands, and the explicit phrase “aggregate 15-second timeout.” | Original implementation and task-specific contract |
| Task 062, commit `d7167fc30531a752b7b1fae0b4b5fca05278eaf4`, “feat: add staged repository diff observer” | Reused `DIFF_TIMEOUT`, added pre/post HEAD checks around raw/numstat/patch, and passed one start instant to all five calls. The Task 062 plan says Task 061 bounds are unchanged. | Staged-diff extension; historical contract |
| Task 359, commit `02c3eed1c865e38a635ee8ba0f3f6caee1d27a16`, “feat: add linked worktree repository identity support” | Added repeated fixed semantic layout validation in repository observers. Each probe has its own five-second timeout. The 15-second aggregate was not revisited. | Later implementation hardening |
| Task 360 and ADR 0029 | Preserve complete identity/currentness evidence and state that no race-free TOCTOU or external-Git exclusion is claimed. | Accepted identity/security decision and audit |

The original motivation evidenced in Task 058/061 is a bounded, fail-closed
observer: fixed commands, bounded output, no partial result on timeout, and a
total time bound. The records do not provide benchmark data or a special
Windows exception explaining why 15 seconds was selected. “Operation,”
“process,” and “observation” are not interchangeable here: the original diff
contract expressly says aggregate/total, while each child is also separately
subject to process supervision.

`README.md`, current `docs/ARCHITECTURE.md`, current `docs/SECURITY.md`, and
accepted repository ADRs do not define a different staged-diff deadline or a
per-phase refresh rule. They establish bounded execution, fail-closed
observation, host-owned repository identity, and the limited scope of the
lease. The detailed timeout unit is therefore selected by the original
task-specific observer contract, not by a newer architecture statement.

The relevant documentation classifications are:

| Material | Relevant statement | Classification |
| --- | --- | --- |
| Current `README.md` | General release orientation and capability summaries; no numeric staged-diff deadline. | No timeout policy stated |
| Current `docs/ARCHITECTURE.md` | Repository identity and currentness remain capability-specific; no staged-diff timeout value or refresh rule. | Normative for architecture boundaries; silent on timeout unit |
| Current `docs/SECURITY.md` | Host owns executable, arguments, cwd, environment, timeout and result limits; no staged-diff numeric or aggregate rule. | Normative for host ownership and bounded execution; silent on timeout unit |
| ADR 0027 / ADR 0029 | Host-owned repository identity/currentness, one active repository, fixed bounded probes, and no cross-process lock or race-free TOCTOU claim. | Normative for authority and identity; silent on staged-diff timeout unit |
| Task 058 research policy table | `repo.diff` and `repo.diff-staged` are listed at 15 seconds total; timeout rejects without partial output. | Historical evidence of the observer design contract |
| Task 061 implementation plan | Names one aggregate 15-second timeout for the three-command diff sequence. | Historical evidence of the original total-budget contract |
| Task 062 staged-diff plan | Reuses Task 061 bounds while adding two HEAD checks and retaining one lease over all five phases. | Historical evidence extending the total-budget contract |
| Task 359 plan | Documents eight fixed layout probes, each with a five-second process cap, and retention/revalidation of layout identity. | Historical implementation description; does not state whether the old aggregate must change |
| Current `RepositoryObserver::run` comment/error/source | Says “common timeout” and emits “repository observation exceeded its total timeout”; checks shared elapsed time after `validate_git`. | Implementation description; source ordering establishes validation consumes elapsed time, while the absence of a passed remaining deadline makes enforcement ambiguous |
| Task 414 / Task 415 records | Task 414 isolates timeout exhaustion; Task 415 stops before redefining the budget. | Historical evidence of the observed failure and unresolved enforcement gap |

No accepted ADR establishes a competing timeout policy. The strongest
timeout-specific evidence is the Task 058/061/062 total-budget contract.

## Current runtime graph

For `DiffBaseline::IndexVsHead`, current source executes:

```text
RepositoryDiffStagedTool::execute
  -> execute_fixed_diff
       -> acquire shared per-root RAH lease
       -> execute_fixed_diff_while_leased
            -> observer.revalidate()                 [filesystem identity]
            -> started = Instant::now()
            -> pre-HEAD observer
                 -> RepositoryObserver::run
                      -> observer.revalidate()        [filesystem identity]
                      -> repository.validate_git()
                           -> layout revalidate
                           -> 8 fixed Git semantic probes
                           -> layout revalidate
                      -> remaining DIFF_TIMEOUT check
                      -> fixed rev-parse HEAD child
            -> raw staged-diff observer
                 -> same validation and deadline sequence
                 -> fixed git diff --cached --raw child
            -> numstat staged-diff observer
                 -> same validation and deadline sequence
                 -> fixed git diff --cached --numstat child
            -> patch staged-diff observer
                 -> same validation and deadline sequence
                 -> fixed git diff --cached --patch child
            -> post-HEAD observer
                 -> same validation and deadline sequence
                 -> fixed rev-parse HEAD child
            -> compare pre/post HEAD; reject if changed
            -> observer.revalidate()                 [filesystem identity]
            -> bounded raw/numstat/patch correlation and normalization
```

`RepositoryObserver::run` picks `DIFF_TIMEOUT`, checks elapsed time only after
`repository.validate_git`, and gives the fixed observation child the remaining
time. The staged sequence supplies the same `Instant` for all five phases.
HEAD commands use the existing fixed `rev-parse --verify -q HEAD` policy and
are part of the same budget. A normal-to-unborn, unborn-to-normal, or changed
HEAD rejects the staged observation.

### Exact process counts

One successful staged observation has five observer phases:

1. pre-HEAD;
2. raw staged diff;
3. numstat staged diff;
4. patch staged diff; and
5. post-HEAD.

Every `RepositoryObserver::run` invokes `RepositoryGitLayout::validate_git`,
which launches exactly eight fixed probes:

1. `rev-parse --show-toplevel`;
2. `rev-parse --path-format=absolute --absolute-git-dir`;
3. `rev-parse --path-format=absolute --git-common-dir`;
4. `rev-parse --is-bare-repository`;
5. `rev-parse --show-superproject-working-tree`;
6. `rev-parse --path-format=absolute --git-path index`;
7. `rev-parse --path-format=absolute --git-path HEAD`; and
8. `worktree list --porcelain -z`.

Thus the successful path has exactly 40 layout-validation Git child processes
and 5 observation Git child processes, for **45 fixed Git child processes in
total**. The 5 observation children comprise 2 identity/currentness HEAD
commands and 3 staged-diff data commands. There is no extra Git child for
`observer.revalidate()`: those calls recheck filesystem identity and retained
layout evidence. The eight probes also revalidate semantic repository
identity/currentness before each observation command.

All eight layout probes have an individual `PROBE_TIMEOUT` of five seconds.
The observation child receives remaining time from the 15-second deadline.
Currently the eight probes are not themselves given that remaining time.
Consequently 15 seconds is the intended aggregate bound, but the current
implementation can spend up to 40 seconds in the validation preceding the
first observer timeout check (in addition to time already spent) before it
returns the aggregate-timeout error. Successful observations remain governed
by the shared deadline checks; timeout failures may overrun because the probe
sequence crosses the deadline before the check. Per-child process supervision
and finite probe count remain in force; no absolute bound on synchronous
filesystem calls or OS scheduling latency is claimed.

## Why validation repeats and what the lease guarantees

Repeated semantic validation is part of the linked-worktree identity
hardening. Each pass checks the retained selected root, executable, `.git`
form, private/common Git directories, selected index and HEAD paths, non-bare
and non-submodule classification, and registration evidence. The before/after
filesystem revalidation checks retained object identities and exact
relationship evidence. This is relevant to detecting repository path or
`.git` replacement, linked-worktree private/common/registration/backlink or
`commondir` changes, and semantic identity substitution between commands.
Those cases are evidenced by `RepositoryGitLayout`, Task 359/360, and ADR
0029. Validation does not make the multi-command read race-free.

The repository lease is a process-local async mutex shared by RAH capabilities
keyed to the selected canonical root (ASCII case-folded on Windows). It
serializes cooperating RAH operations on that root, including RAH Stage,
Unstage, and observations that use the same lease registry. It does not:

- prevent another process or external Git client from changing repository
  files, Git metadata, HEAD, refs, or the index;
- pin the filesystem root or `.git` objects with open handles;
- prevent root, `.git`, private/common directory, or linked-worktree
  registration replacement;
- serialize separate linked-worktree roots merely because they share a common
  Git directory; or
- provide a transaction, rollback, or race-free TOCTOU guarantee.

Therefore repeated validation remains security-relevant. Design D below is
rejected. The accepted policy preserves identity validation before every
fixed observation command and the existing lease ownership.

## Timeout classification and chronology assessment

`DIFF_TIMEOUT` is primarily a **composite resource/liveness bound**. It limits
the aggregate fixed Git observation sequence and causes fail-closed
unavailability when work does not finish in time. Child execution also has a
process-execution bound: observer commands receive remaining aggregate time,
and validation probes have an individual five-second cap. This timeout does
not grant or change repository authority.

The commit chronology supports an emergent mismatch between individually
reasonable changes. Task 061/062 established the 15-second aggregate before
linked-worktree semantic validation existed. Task 359 later added eight
sequential probes before each fixed observer command without revisiting the
older aggregate budget. Task 414 reproduced the resulting timeout before a
fixed observer child was spawned. This is an expansion of bounded work inside
an older aggregate policy, not evidence that the intended high-level timeout
was changed to per-command.

## Candidate policy assessment

Let `N = 5` observer phases; each phase currently has 8 validation probes and
one fixed observation child.

| Design | High-level bound | Assessment |
| --- | --- | --- |
| **A. One aggregate 15-second total** | Intended: `T_total <= 15s`. Current probe enforcement may overrun by at most one sequential validation's `8 × 5s = 40s`, subject to unbounded OS/filesystem scheduling latency; Task 417 must clamp probes to the shared remaining time. | **Chosen.** Matches Task 058/061/062 aggregate/total contract, retains a single fail-closed observation bound, and does not expand slow-repository exposure. Ordinary scheduling pressure may cause bounded unavailability, which the observer contract permits. |
| **B. Fresh budget per observer** | `T_total <= N × 15s = 75s`, if each full phase strictly enforces its own 15-second validation-plus-child budget. | Rejected. It materially expands the total resource/liveness allowance fivefold and allows repeated slow validation. No timeout history or docs authorize refreshing the budget per phase. |
| **C. Separate validation and diff budgets** | `T_total <= T_validation + 15s`; if bounded only by existing per-probe caps, validation is at most `N × 8 × 5s = 200s`, so `T_total <= 215s` before scheduling/filesystem latency. A tighter separate aggregate validation cap would require a new selected value. | Rejected. It introduces a second policy surface and can greatly expand runtime. No existing evidence selects a separate validation budget or value. Repeated validation itself remains necessary. |
| **D. Validate once, then run sequence** | With one validation under current per-probe caps: at most `8 × 5s + 15s = 55s`, subject to scheduling/filesystem latency. | Rejected as security-unsafe. The lease excludes external actors, so one validation cannot detect later root/layout/registration substitution between fixed commands. It weakens the existing per-command identity check. |
| **E. Allocate slices/reserves under one 15-second total** | `T_total <= 15s` if every slice is strictly enforced. | Not selected. It preserves the same aggregate bound but adds phase scheduling and reserve policy without evidence that fixed slices are needed. A single absolute deadline passed through validation and observation commands is simpler and directly implements the historical aggregate contract. |

For Design A, the aggregate begins before pre-HEAD and ends after the post-HEAD
fixed observer command completes or the sequence fails. Repository validation
counts against the same deadline; no refresh occurs. Each child timeout is
`min(existing per-child cap, remaining aggregate time)`. Timeout exhaustion
returns the existing fail-closed observation error. If validation reaches the
deadline, no subsequent fixed observation child may spawn.

## Authority, security, and error effects

The chosen policy changes timeout enforcement only. It changes no
`RepositoryObservation` classification, `ToolRegistry` authority,
`HostExplicit` membership, repository admission, selector/currentness rules,
or lease ownership. It preserves eight probes before each observer child and
the current fixed Git command vectors. It does not weaken validation or claim
external-process exclusion. It does reduce the current validation overrun by
making probes obey the existing aggregate deadline.

The candidate authority/security comparison is:

| Candidate | Repository authority / `RepositoryObservation` / `ToolRegistry` / `HostExplicit` / admission | Lease and validation | TOCTOU effect |
| --- | --- | --- | --- |
| A | No change | Same lease and repeated validation | Same evidence/check frequency; strict enforcement of existing total |
| B | No formal authority change | Same lease/validation, but a fresh deadline at each phase | No added check gap; expands duration and slow-work exposure |
| C | No formal authority change | Same lease/validation plus a new budget policy | No added check gap; expands duration by the separate validation allowance |
| D | No formal authority change | Same lease, but only one semantic validation | Larger unchecked interval between commands; rejected |
| E | No formal authority change | Same lease/validation, with deadline slices | No check gap, but adds scheduling policy without evidence |

On timeout, `RepositoryObserver::run` continues to return the existing
repository observation total-timeout error. The staged tool continues to
surface execution failure; Desktop maps it to `StagedDiffExecution` for direct
snapshot collection and to its existing empty `ReviewUnavailable` fallback
during refresh. Stage/Unstage review-dependent action availability remains
unavailable when the required staged observation fails. No partial staged
diff is returned and no selector/currentness decision is changed.

A timeout on a valid but slow or heavily scheduled repository is normal
bounded unavailability under the fail-closed observer contract. It is not
permission to retry without bounds or to publish a partial review. The
intermittent test failure is evidence that the selected bound was reached; it
does not establish a nonzero Git exit, a repository-layout defect, or a
selector/currentness defect.

## Release and test implications

The v0.32 release must not proceed while its required validation remains
failing. Retaining the intended aggregate policy does not make the Task 412
gate pass automatically. Task 417 must establish deterministic evidence that:

- each semantic probe and fixed observation child receives no more than the
  remaining aggregate time;
- an exhausted deadline prevents the next child from spawning and maps to the
  existing unavailable/error path;
- all five phases share one deadline, including all 40 possible validation
  probes; and
- a successful path preserves HEAD checks, fixed command semantics, repeated
  validation, and lease ownership.

The timing regression must not sleep for 15 seconds. The current code has no
clock/probe seam for deterministic elapsed-time advancement. Task 417 may add
the minimum private test seam needed to inject a monotonic clock or bounded
probe runner; it must not add a public timeout API. A deterministic fake must
prove validation consumes the same budget and that no later process is
started after exhaustion. A focused Windows stress run may then establish
whether ordinary repository fixtures complete inside the selected aggregate
limit. Existing tests must distinguish allowed `ReviewUnavailable` from
assertions that specifically require a successful observation; they may not
silently turn the aggregate deadline into a per-phase deadline.

Task 412's currently failing release suite remains a release blocker until
the selected behavior is implemented/enforced and its documented gate passes.
This conclusion is based both on the explicit release gate and on product
behavior: timeout is an accepted fail-closed unavailable state, but the release
cannot claim successful validation while required tests intermittently fail.

## ADR assessment

```text
NO ADR — clarification and enforcement of the existing aggregate observer policy
```

Task 058/061/062 already document a total aggregate observation timeout.
Enforcing the same total across later-added layout validation does not change
authority, validation strength, process types, or the timeout value. Task 417
must preserve this scope. If implementation research finds that a second
validation budget or a longer/per-phase total is necessary, stop and request a
separate architectural decision; that would change the bounded-runtime
contract and is outside this clarification.

## Canonical staged-diff timeout policy

1. **Unit:** one complete staged `IndexVsHead` Git observation sequence.
2. **Budget:** the existing `DIFF_TIMEOUT = 15 seconds`; it is one aggregate
   deadline, not a per-command or per-observer allowance.
3. **Start:** immediately before the pre-HEAD observer phase in
   `execute_fixed_diff_while_leased`.
4. **Included work:** repeated filesystem/layout revalidation, all eight
   semantic Git probes before each of the five observer phases, the two HEAD
   identity commands, and the raw, numstat, and patch staged-diff commands.
5. **Refresh:** never. Every phase receives the same deadline.
6. **End:** completion of the post-HEAD observation child, or the first
   timeout/error. Bounded in-process output correlation remains under its
   existing output/file caps after the Git observation sequence.
7. **Child relationship:** each probe retains its existing five-second
   ceiling; each fixed observer child retains its existing policy but is
   clamped to the shared remaining aggregate duration. No child may be
   launched after the deadline.
8. **Exhaustion:** return the existing fail-closed repository observation
   timeout error; return no partial staged review, retry, or selector update.
9. **Maximum:** the intended Git observation bound is 15 seconds, excluding
   unavoidable OS scheduling and synchronous filesystem latency not governed
   by child-process timeout controls.

## Exact Task 417 boundary

Task 417 is an implementation task for this policy only:

- **Source files:** `crates/rah-tools/src/repository_observer.rs` and
  `crates/rah-tools/src/repository_git_layout.rs`; `repository_diff.rs` may
  change only if needed to carry an absolute deadline without moving its
  existing start point.
- **Symbols:** `DIFF_TIMEOUT`, `RepositoryObserver::run`,
  `RepositoryGitLayout::validate_git`, and its private `probe` helper. Keep
  `execute_fixed_diff_while_leased` as the owner of the one staged-diff
  deadline.
- **Deadline:** start before pre-HEAD, end after post-HEAD child or first
  failure; pass remaining time into every validation probe and observation
  child; do not reset per phase.
- **Constants:** retain `DIFF_TIMEOUT = 15s` and `PROBE_TIMEOUT = 5s` as
  maxima. A child receives the lesser of its existing cap and remaining total
  time. Do not increase either constant.
- **Git and validation invariants:** retain the exact 8 validation probes,
  exact 5 observation command roles, their fixed argv/environment/output caps,
  validation before every fixed observer command, all filesystem identity
  checks, pre/post HEAD equality, and one shared RAH lease.
- **Authority and error invariants:** no authority, admission,
  `RepositoryObservation`, selector/currentness, fallback, or error-category
  changes. Exhaustion remains fail-closed with no partial result.
- **Maximum:** one aggregate 15-second Git observation deadline. No per-phase
  refresh and no separate budget.
- **Deterministic regression:** use a private fake clock/probe runner or
  equivalent seam; prove the same deadline is consumed by validation and
  commands, and that timeout prevents a subsequent child. Do not use a real
  15-second sleep.
- **Stress evidence:** after deterministic tests, repeat the affected serial
  Windows Desktop tests/package stress sufficiently to show whether normal
  fixtures stay within the preserved total. Record remaining bounded
  unavailability as such; do not weaken the contract to make tests pass.

## Non-goals

- No Rust or test edits in Task 416.
- No timeout constant, Git command, repository validation rule, selector,
  currentness, lease, admission, or authority change.
- No retry, sleep, unbounded execution, per-observer deadline refresh, or
  separate validation budget.
- No Task 412 validation resumption, commit of the eight release-preparation
  files, push, tag, or automatic start of Task 417.

## Task 416 closure evidence

- Starting HEAD: `db0d99c2bdda3f5264ad71f960105da5de40cf6d`.
- Starting Task 412 fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.
- Ending Task 412 fingerprint: required to remain
  `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3` and checked before commit.
- Task 415 record: `docs/plans/2026-09-25-task-415-staged-diff-observation-timeout-correction.md`.
- Task 416 record: `docs/plans/2026-09-25-task-416-staged-diff-timeout-policy-design.md`.
- Validation: documentation-only; `git diff --check` only. No Cargo or live
  validation was run.
- Commit: exact docs-only commit recorded in the final Task 416 report.
- Push/tag: none.
- Recommended next task: Task 417 — enforce the existing shared 15-second
  staged-diff aggregate deadline through repository-layout validation and
  fixed observer commands, then run its deterministic timeout regression and
  focused Windows stress evidence.
