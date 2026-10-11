# Task 418 — Staged-Diff Aggregate Timeout Viability and Policy Revision Research

Date: 2026-09-25

Status: **STOP — STAGED-DIFF TIMEOUT VIABILITY REMAINS UNRESOLVED**

## Starting checkpoint and frozen work

- Starting HEAD: `1210e0cbc6bdcc4fd7395896cb1b53f2f9524b65` (`docs: clarify staged diff timeout policy`). The index was empty.
- Task 412 eight-path binary-diff fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.
- Task 417 three-Rust-file binary-diff fingerprint: `0525250ea4936b0baa37451bc7962bfbecf17af6`.
- The main worktree's Task 412 release preparation and Task 417 failed implementation/evidence were left untouched. No release preparation was resumed.

## Failure trace and static work audit

The failed Task 417 `rah-tools` library run reported 341 passed, 2 failed, 0 ignored in 949.30 seconds. Both failures were `Execution { message: "Git repository policy rejected capability: repository observation exceeded its total timeout" }`. Their exact tests are in `crates/rah-tools/src/repository_commit.rs`:

| Test and range | Fixture and staged content | Review path and expected result |
| --- | --- | --- |
| `binary_staged_content_never_produces_or_arms_a_review`, lines 1738–1796 | `fixture()` initializes a normal repository with one committed `tracked.txt`; it adds `binary.dat` containing `00 9f 92 96`. | `RepositoryCommitControl::review_current_staged_snapshot` should return a binary presentation and no opaque review; later authorization should fail without arming or changing HEAD, refs, index, or bytes. The prior timeout occurred at the line 1769 review unwrap. |
| `authorizing_an_opaque_review_never_invokes_commit`, lines 1891–1948 | Same normal repository fixture; `stage()` replaces and stages `tracked.txt` with `reviewed\n`. | A current staged review should yield an opaque review; authorizing it should leave HEAD, status, and commit count unchanged. The prior timeout occurred at line 1917 review unwrap. |

`RepositoryCommitControl::review_current_staged_snapshot` (lines 610–614) calls `RepositoryCommitPolicy::review` (lines 181–207), which captures a before snapshot, builds `RepositoryObserver`, and calls `execute_fixed_diff_while_leased(observer, IndexVsHead)` (line 186). The latter starts one `Instant` at `repository_diff.rs:102`, then runs pre-HEAD, raw, numstat, patch, post-HEAD through `RepositoryObserver::run_diff`. That runs `RepositoryIdentity::validate_git_with_budget` and `RepositoryGitLayout::validate_git_with_budget` before each fixed child. The timeout returns through the `?` chain to the review unwrap. These commit tests use the private fixed staged observer directly; `RepositoryDiffStagedTool::execute` is **not** reached. No commit child is invoked at this failure point.

The frozen Task 417 diff adds budget propagation and a pure deadline helper. It preserves five observer command roles, eight layout probes per phase, all fixed Git argv, repeated validation, probe ceiling `5s`, `DIFF_TIMEOUT = 15s`, timer start, leases, selector/currentness, and error mapping. It adds no Git child, duplicated command, duplicated validation pass, retry, or sleep. Static audit therefore finds **no unintended execution work**. The changed call to `run_diff` still selects the same five command roles. The `run_with_budget` command clamp only reduces or preserves a child's allowance.

One pre-existing semantic detail remains material: `RepositoryObserver::run_with_budget` selects `FILE_INFO_TIMEOUT = 5s` for both HEAD commands and subtracts the **shared observation start** (`started.elapsed()`) before launching each. Therefore the post-HEAD command has at most `5s - elapsed_since_pre_HEAD` remaining even though the staged diff's nominal aggregate limit is 15s. This exists in the clean baseline too; Task 417 did not add it. It may cause an earlier fail-closed timeout, but no captured failed run identifies HEAD as the exhausted phase.

## Controlled A/B setup and reproduction

Two disposable linked worktrees at the same HEAD were used. The baseline was clean pre-enforcement source. The enforced worktree received only the three frozen Task 417 Rust/test file diffs; its Rust diff rehashed to `0525250ea4936b0baa37451bc7962bfbecf17af6`. Task 412 files and Task 417 documentation were not applied. Both used the same Windows host, native Git, and serial test settings, sequentially. **Separate Cargo target directories were required:** an initial shared-target baseline attempt silently reused the enforced 343-test binary despite clean baseline source. That attempt's five-by-two focused matrix and full package pass were invalid as baseline evidence and are excluded. The valid isolated baseline binary reports 340 library tests (339 filtered in each focused run). The first enforced and isolated-baseline focused commands included compilation time; test-harness durations below exclude that setup and must not be mistaken for observation time.

Commands used for each named test were `cargo test -p rah-tools --lib <fully-qualified-name> -- --exact --test-threads=1 --nocapture`, five runs each. All ten enforced and all ten baseline runs passed:

| Source | Opaque review runs 1–5: test harness seconds | Binary review runs 1–5: test harness seconds | Timeout failures |
| --- | --- | --- | --- |
| Enforced | 13.33, 13.63, 12.87, 13.17, 12.76 | 11.38, 11.62, 11.20, 11.30, 11.34 | 0/10 |
| Baseline, isolated target | 13.73, 12.64, 12.31, 12.31, 12.54 | 11.12, 11.31, 11.26, 11.26, 11.35 | 0/10 |

The identical uninstrumented package command was `cargo test -p rah-tools -- --test-threads=1`:

| Source and run | Result | Library result | Package total across 18 binaries | Full command wall seconds | Target failures |
| --- | --- | --- | --- | ---: | --- |
| Enforced 1 | PASS | 343 passed, 0 failed | 451 passed, 0 failed, 2 ignored | 1116.817 | Neither target failed |
| Baseline 1, isolated target | PASS | 340 passed, 0 failed | 448 passed, 0 failed, 2 ignored | 1174.127 | Neither target failed |
| Enforced diagnostic 2, with temporary timing prints and `--nocapture` | PASS | 343 passed, 0 failed | 451 passed, 0 failed, 2 ignored | 1032.414 | Neither target failed |

The integration binaries and doc tests passed in each valid full run. The three-test library difference is Task 417's deterministic deadline-helper tests. Two further full uninstrumented runs per source were **not** performed; “up to three” was a ceiling. This bounded sample cannot establish whether a clean baseline can or cannot exhibit the intermittent timeout. Classification from *fresh A/B runs alone*: **NEITHER**. Including Task 417's recorded failed enforced run: enforced has one historical failure and two fresh passes (one diagnostic); baseline has no observed failure in one valid fresh package run. It would be incorrect to classify this as proven “ENFORCED ONLY.”

## Monotonic timing diagnostics

Temporary `eprintln!` diagnostics were added only to the disposable enforced worktree after the uninstrumented comparison. `Instant` timestamps were taken at the existing whole-observation start, before and after each validation pass, around each fixed command, and around every layout `execute_process`. Logs contain fixed command names and elapsed milliseconds, no raw repository paths. The patch did not change timeouts, command order, validation, retry policy, or test assertions. Its log I/O is a small measurement perturbation, so the figures are descriptive, not an uninstrumented latency guarantee.

Five runs each used the two commit-review fixtures plus `repository_diff_staged::index_vs_head_includes_only_staged_changes_and_is_read_only` (an ordinary staged review containing text changes, deletion, and binary content). The binary commit fixture performs two staged observations per test; the opaque fixture and mixed integration fixture perform one each. Thus the focused diagnostics contain 20 successful complete observations and 800 layout probe invocations.

| Fixture | Successful observations | Whole staged observation min / median / max | Five validation passes, sum min / median / max | Five fixed commands, sum min / median / max | 40 probe calls, sum min / median / max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Opaque text commit review | 5 | 1915 / 2046 / 2071 ms | 1701 / 1822 / 1863 ms | 195 / 201 / 249 ms | 1643 / 1756 / 1795 ms |
| Binary commit review | 10 | 1966 / 2018.5 / 2048 ms | 1744 / 1765.5 / 1803 ms | 189 / 217.5 / 287 ms | 1680 / 1702.5 / 1735 ms |
| Mixed staged diff integration | 5 | 2083 / 2095 / 2110 ms | 1851 / 1859 / 1869 ms | 220 / 225 / 227 ms | 1782 / 1795 / 1803 ms |

The combined focused range is 1915–2110 ms, median 2044 ms (20 observations). Validation takes about 85–90% of these whole-observation durations; the fixed observer commands take about 9–14%, and short synchronous work accounts for the remainder. The probes themselves consume nearly all validation time. Forty process launches are the main measured cost. No diff command is individually large in these fixtures.

The separately instrumented serial `rah-tools` package run captured 36 more successful staged observations, 1909–2159 ms, median 2040 ms. Across that run, validation passes ranged 315–492 ms (median 346), fixed command executions 36–92 ms (median 38), and individual layout probes 35–143 ms (median 37). It also captured one incomplete staged observation in `repository_nested_boundary::real_repo_a_and_nested_repo_b_are_rejected_before_observation`: pre-HEAD completed at 394 ms and raw-phase validation completed at 752 ms, after which the expected nested-boundary rejection ended the test successfully. That is not a timeout. The diagnostic log's one `pre_spawn_timeout elapsed_ms=15000` line came from Task 417's deterministic `exhausted_aggregate_budget_rejects_probe_before_spawn` unit test, not a live Git observation. There was no spontaneous timeout in 56 successful complete observations across focused and serial diagnostics.

Individual layout probe durations across 100 calls of each fixed probe (20 observations × five validation passes):

| Probe | Min / median / max ms |
| --- | ---: |
| `rev-parse --show-toplevel` | 35 / 37 / 86 |
| `rev-parse --absolute-git-dir` with fixed absolute-path option | 35 / 37 / 85 |
| `rev-parse --git-common-dir` with fixed absolute-path option | 35 / 37 / 68 |
| `rev-parse --is-bare-repository` | 35 / 37 / 88 |
| `rev-parse --show-superproject-working-tree` | 48 / 51 / 99 |
| `rev-parse --git-path index` with fixed absolute-path option | 35 / 37 / 97 |
| `rev-parse --git-path HEAD` with fixed absolute-path option | 36 / 37 / 85 |
| `worktree list --porcelain -z` | 36 / 37 / 89 |

The superproject probe is consistently a little slower, but no one probe dominates. The 40 similar-sized child executions, rather than one anomalous fixed Git operation, dominate normal elapsed time. The data do not identify an OS service, antivirus, indexing, or other external cause.

A representative successful binary observation showed the following exact cumulative phases, in milliseconds. Each row has eight successful probes and the fixed observer command shown; the remaining value is the effective command allowance after validation, including the HEAD class cap where applicable.

| Phase | Aggregate before validation | Validation duration | Aggregate after validation | Remaining allowance | Command duration | Aggregate after command |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| pre-HEAD | 1 | 317 | 318 | 4681 | 42 | 361 |
| raw | 362 | 349 | 712 | 14287 | 39 | 751 |
| numstat | 753 | 360 | 1113 | 13885 | 40 | 1154 |
| patch | 1155 | 347 | 1503 | 13496 | 51 | 1555 |
| post-HEAD | 1556 | 393 | 1950 | 3049 | 36 | 1987 |

The observation completed at 1987 ms. Its post-HEAD allowance of 3049 ms is `5s - 1950ms`, not `15s - 1950ms`. The aggregate remaining budget at that point was 13049 ms; the HEAD class cap was binding.

The historical Task 417 failure has **no per-phase instrumentation**. Its error proves exhaustion at a fail-closed observation timeout boundary, but the last completed phase, elapsed time, remaining budget, and whether the stop was before/during a layout probe or before/during an observer child were not captured. The fresh timed fixtures did not fail. No failure timing is fabricated from the historical suite duration.

## Policy assessment

**A — keep aggregate 15s.** Normal measured staged observations are around two seconds and do not show that 15s is routinely close. Yet the recorded Task 417 package failure and Task 414 Desktop failures remain real. No specific correctable mechanism has been proven. Thus A cannot be certified as release-ready from this evidence, although no measurement justifies replacing it.

**B — larger single aggregate bound.** The data supply no defensible exact replacement value. The largest healthy focused observation is 2110 ms; a diagnostic serial run may extend that range, but an isolated failure beyond the bound does not give a healthy-tail target. Increasing `DIFF_TIMEOUT` alone would also leave the existing HEAD `FILE_INFO_TIMEOUT` shared-start behavior, which can reject near five seconds. Choosing 30s, 45s, or another round number would be an unsupported policy change and an expanded resource/liveness allowance.

**C — per-observer budget.** This could admit up to five refreshed 15s phases (75s) and would loosen the total without explaining the intermittent stall. Rejected.

**D — separate validation/observation budgets.** Validation is the dominant normal cost, but it performs security-relevant checks before every command. Two budget domains have no measured threshold or security rationale here. Rejected.

**E — reduce validation cost while preserving semantics.** The 40 probes are the main ordinary cost and Task 359 materially shifted the successful-path workload. Combining Git queries while proving equivalent selected-root, private/common Git, index/HEAD, bare/submodule, and registration evidence could be future optimization research. No equivalent design or proof is established here, and no probe was removed. It is not a Task 418 policy selection.

**Canonical policy decision:** no revision is authorized by this measurement. Retain the current 15-second intended aggregate and five-second per-probe ceiling as the research baseline, with repeated validation before each of five fixed commands and fail-closed no-partial-result behavior. This is **not** a new certification that the current implementation reliably completes. The high-level intended bound remains 15s for Git child execution under the shared deadline, excluding uncontrolled synchronous filesystem/scheduler latency; no new worst-case bound was selected. The current HEAD class behavior creates an earlier effective check and needs direct diagnosis before a policy choice.

Task 359 added 40 probes to the successful staged path; these account for most of the measured ~2s normal duration, so the workload shift is material. This record cannot conclude that it made the historical 15s aggregate operationally incompatible with ordinary hardened observation. The observed failures are extreme relative to sampled successes, and their phase/cause is unresolved.

## Security, ADR, release, and next boundary

There is no authority, `ToolRegistry`, `HostExplicit`, repository admission, lease, selector/currentness, or validation-rule change in Task 418. No timeout, command, or production source was changed permanently. Resource/liveness policy remains unresolved; Task 412 remains **BLOCKED**. The release blocker classification is **D — unresolved**. It is neither a proven Task 417 work defect, proven obsolete 15s value, nor an identified independent runtime mechanism.

**Task 417 disposition: KEEP AS BASIS FOR NEXT IMPLEMENTATION.** Its shared-deadline propagation adds no execution work and enforces the intended probe clamp. Keep it uncommitted and unchanged in the main worktree. If later diagnostics establish that the shared-start HEAD class cap is the cause, a narrow correction can preserve the 15s aggregate while giving each HEAD child its existing five-second ceiling from child launch, clamped to aggregate remaining. That correction is a candidate, not an authorized Task 418 code edit.

**ADR assessment:** no ADR is created for an unresolved policy. A later selected aggregate increase or separate budget is a new bounded-runtime decision and requires an ADR under Task 416's rule. The ADR must record the historical 15s policy, Task 359's 40-probe expansion, Task 414/417 failure evidence, measured healthy and failed tails, exact selected bound, security/liveness effects, unchanged authority semantics, and rejected alternatives. No existing higher-level policy was found to authorize a larger bound.

**Exact recommended Task 419 boundary:** `Task 419 — Localize Intermittent Staged-Observation Timeout and Resolve HEAD Budget Semantics`. Preserve the Task 417 diff as the candidate base; capture the last completed phase and pre/during-probe versus pre/during-command timeout in bounded serial stress, check the `FILE_INFO_TIMEOUT` shared-start effect with deterministic tests, and determine whether the cause is an incorrect child clamp or a measured healthy runtime tail. Do not select a larger aggregate until evidence supports an exact value. If a larger/separate budget is later selected, create the required ADR before implementation, keep all 40 probes and five observer commands, test fail-closed behavior, run `rah-tools` and Desktop A/B/C/D and package stress, then obtain independent audit before Task 412 resumes. No Task 419 work began here.

## Closure checks

- Main-worktree `git diff --check`: PASS (Git emitted only existing LF/CRLF warnings). The new untracked artifact also has no trailing whitespace by direct scan.
- Ending Task 412 fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`, unchanged.
- Ending Task 417 implementation fingerprint: `0525250ea4936b0baa37451bc7962bfbecf17af6`, unchanged.
- Disposable timing edits were reverted; the enforced disposable diff matched the frozen Task 417 fingerprint before both worktrees were removed. `git worktree list` again showed only the main worktree.
- No file is staged. The main worktree contains the original eight Task 412 paths, the three Task 417 Rust paths, the Task 417 plan, and this new Task 418 artifact. No Rust or tests were changed permanently. No docs-only commit, push, or tag occurred.
- Automatic approval review rejected recursive removal of the separate baseline Cargo build cache outside the repository. That cache remains; it is build output only. Diagnostic logs also remain outside the repository for review.
