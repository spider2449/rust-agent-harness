# Task 504B — Natural failure evidence capture

**C — FAILURE NOT REPRODUCED WITHIN BOUNDED MATRIX.**

Starting and final HEAD: `3ab73e32d33b37acb8027d7ad4b4c7b8e078d077`.
No natural failing invocation was captured. This does not prove that the
historical failure disappeared permanently or that instrumentation had no
timing effect. Task 504's historical failure and stopped validation remain.

## Checkpoint and preserved WIP

Used the existing `F:/coding/otherPrj/rust-agent-harness` worktree on master.
Before instrumentation, `git status --short` and `git diff --check` were
recorded in the evidence directory; the latter exited 0 (line-ending warnings
only). The authoritative checkpoint matched the full 40-character HEAD.

Task 504 WIP remained dirty and uncommitted: seven tracked source files modified
(Desktop main.rs/main_tests.rs, Codex experimental.rs/experimental/tests.rs/runtime.rs,
runtime experimental.rs/experimental_host.rs), plus untracked Desktop
runtime_composition.rs, ADR 0033 and the Task 504 plan. The existing Task 504A
report was also untracked. No reset, stash, clean, rebase, worktree recreation,
source correction, Desktop migration validation or alternate-provider work ran.

Evidence directory: `F:/temp/rah-task504b-evidence`.
Full per-invocation Cargo logs, machine-readable counts and timestamps are in
`matrix-results.json` and `matrix-{A|B|C}-{run}.log`. PowerShell native output
redirection retains logs as UTF-16LE; the NativeCommandError formatting around
Cargo's initial stderr compilation output is PowerShell formatting, not a Cargo
failure. Every Cargo exit code was zero.

## Temporary diagnostic instrumentation

Original bytes were backed up before editing. Exact SHA256 manifests are
`source-before.json` and `source-frozen.json`. The complete temporary patch
is retained outside the worktree in `diagnostic-instrumentation.patch`.

- bridge.rs: test-only accepted authorization/canonical Tool definition,
  permission set, ToolCall arguments and session/correlation identity; complete
  typed completion result before AuthorizedDispatchError::Tool erasure; final
  bridge projection. AuthorizedDispatchError itself was not redesigned.
- bridge_tests.rs: test-only panic-unwind fixture root and filesystem metadata
  before the existing Drop cleanup. No cleanup was skipped or deferred. No
  fixture failed, so this failure-only path did not run.
- repository_search.rs and repository_list.rs: debug-build Tool entry, bound
  repository root and actual input, independently for each implementation.
- repository_observer.rs: fixed observer result/error with exit, timeout,
  overflow and output lengths; raw stderr contents excluded.
- repository_git_layout.rs: fixed probe root/arguments, typed process error,
  exit, timeout, overflow, termination flag and complete already-bounded stdout
  bytes, plus stderr length. No raw stderr contents were printed.
- repository_create_directory.rs: Tool entry, capture_start, revalidate_start,
  native_create; typed errors retained immediately before conversion to ();
  boundary/target and path/parent/Git equality rejection labels.

Directory capture_map identifiers correspond to the original expressions:
1 repository_ok; 2 validate_directory_path(parent); 3 reject_reparse_ancestry(parent);
4 FileIdentity::capture(parent); 5 parent.strip_prefix(root);
6 NativeParent::open; 7 git_snapshot(layout). capture_option labels distinguish
the path.parent and UTF-8 file-name Option failures by retained source line.
capture_boundary identifies nested-boundary rejection before the original
short-circuited target-existence check; capture_boundary_or_target prints safe
path/parent/target metadata on rejection. revalidate_equality records original
captured and current path, parent identity and Git snapshot values only upon
the original equality rejection. These labels describe existing expressions,
not new policy stages.

Tool-layer hooks use cfg(debug_assertions), since rah-tools is compiled as a
dependency without cfg(test) for Codex library tests. All hooks are temporary.
The bridge/fixture hooks use cfg(test). Output is private Cargo-test evidence,
not an external authority or contract. No permission, binding, predicate,
timeout, retry, result mapping or lifecycle decision was changed. Failure-only
metadata reads would occur after rejection or during unwind. Logging adds work
and may perturb timing; these passes are not production stability certification.

After instrumentation, git diff --check passed and the seven touched files were
hashed. Source was frozen throughout one matrix epoch. Before every invocation,
the driver compared those hashes and would stop on any mismatch. It checked
them again before restoration. No source edits occurred during Cargo execution.
Normal harness capture retained diagnostics for failing tests; successful
test diagnostics were suppressed by the harness. Thus the full Cargo logs are
not claimed to contain per-invocation successful Tool traces.

## Bounded reproduction matrices

Both CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR were
`F:/temp/rah-task504-target`, the existing Task 504 target.
No isolated single-test invocation was performed in Task 504B.

| Matrix | Exact command | Completed | Result |
| --- | --- | --- | --- |
| A | cargo test -p rah-runtime -p rah-runtime-codex | 8/8 | all exit 0 |
| B | cargo test -p rah-runtime-codex --lib | 8/8 | all exit 0; default parallelism |
| C | cargo test -p rah-runtime-codex --lib -- --test-threads=1 | 4/4 | all exit 0; serial |

Each A invocation had rah-runtime counts 8+6+1+3 = 18 passed; Codex library
102 passed, 0 failed, 1 ignored; the other Codex test binaries had 6 and 11
passed; both doc-test groups had 0 tests. Total per A invocation: 137 passed,
0 failed, 1 ignored. Each B and C invocation had 102 passed, 0 failed,
1 ignored. Every historically failing test passed in every library run.

| Run | Exit | Complete test-result counts | Evidence log |
| --- | --- | --- | --- |
| A1 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 50.46s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-1.log |
| A2 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 49.03s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-2.log |
| A3 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 48.26s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-3.log |
| A4 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 50.84s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-4.log |
| A5 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 48.50s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-5.log |
| A6 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 47.31s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-6.log |
| A7 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 47.40s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-7.log |
| A8 | 0 | 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 48.60s<br>6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s<br>11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | matrix-A-8.log |
| B1 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 51.75s | matrix-B-1.log |
| B2 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 48.78s | matrix-B-2.log |
| B3 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 47.60s | matrix-B-3.log |
| B4 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 47.56s | matrix-B-4.log |
| B5 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 50.68s | matrix-B-5.log |
| B6 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 47.65s | matrix-B-6.log |
| B7 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 50.61s | matrix-B-7.log |
| B8 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 49.77s | matrix-B-8.log |
| C1 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 155.14s | matrix-C-1.log |
| C2 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 152.54s | matrix-C-2.log |
| C3 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 152.83s | matrix-C-3.log |
| C4 | 0 | 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 152.53s | matrix-C-4.log |

No first natural failing invocation exists within this matrix. No additional
reproduction attempts, targeted reruns, full workspace validation or Desktop
gates were run after these bounds.

## Underlying evidence and classifications

| Historical test | Task 504B natural failure evidence | R code |
| --- | --- | --- |
| repository_search_dispatches_through_the_generic_bridge | None; historical ToolError variant/reason remains unknown | unclassified |
| repository_list_dispatches_through_the_generic_bridge | None independently; historical ToolError variant/reason remains unknown | unclassified |
| host_composed_repo_create_directory_uses_generic_bridge_once | None; historical exact capture/revalidation stage and predicate remain unknown | unclassified |

Task 504A's independent typed-branch findings remain: search/list reached
AuthorizedDispatchError::Tool, and directory returned precondition_failed,
uncertain:false before native creation. This task did not recover the discarded
historical source or reproduce the rejection. ToolError's existing variants
carry strings or ToolName, with no nested typed source chain; the diagnostics
inspect Debug data before mapping rather than relying on Display alone.
Neither R1/R2 nor R3–R7 is assigned without a demonstrated cause.

**Shared-root conclusion: NOT_PROVEN.** Parallel and serial executions all
passed. This provides no failed-vs-successful comparison and proves neither
interference nor its absence. No failure-triggered shared-state audit was
expanded. No failing fixture existed to inspect before cleanup; expected/actual
failure filesystem state and repository status therefore remain unavailable.
No fixed-path collision, environment mutation, shared observer state, ordering
interaction or globally cached state was proven by this task.

**V1/V2/V3/V4: unclassified**, as required when no natural failure reproduces.
These legacy-only paths do not supply new contradictory evidence against
Task 504A's host-port/lifecycle findings.

**Bounded correction recommendation:** none is justified. Do not correct
production behavior, relax preconditions/deadlines or revise fixtures on these
passes. A future separately authorized diagnostic task would need evidence
from an actual failed invocation before selecting a correction. The already
established source erasure is a separate observability concern: consider a
separately scoped review of retaining a sanitized diagnostic plus private typed
Tool source. Task 504B neither changes nor approves a durable external error
contract redesign.

## Restoration and closure

All temporary instrumentation was removed by restoring exact saved bytes after
the matrix process exited. Seven of seven post-restoration SHA256 hashes equal
their pre-instrumentation values (source-restoration.json). Diagnostic-touched
source has no diff against HEAD. Task 504 product WIP remains preserved.

| Diagnostic-touched source | Pre-instrumentation and restored SHA256 | Equal |
| --- | --- | --- |
| crates/rah-runtime-codex/src/bridge.rs | `54ca0cf9a6cdca31844f09f60048a39811d96ac3acca2496a5f15533fdfa4e01` | yes |
| crates/rah-runtime-codex/src/bridge_tests.rs | `5eb4b6bf408434681b5541533772fe0b74d565de9a4dcbafaf536748813b37a6` | yes |
| crates/rah-tools/src/repository_search.rs | `f61cd5f845a2cc40fd8f1b17a449565bec08fd5603fa475ed6663ccdcbc403e1` | yes |
| crates/rah-tools/src/repository_list.rs | `6c9e7f1c2a5dd909f7404952d882fdf7672423ec8eb6b1a2475eb1c66680fc5f` | yes |
| crates/rah-tools/src/repository_create_directory.rs | `19cad743229eb8c4bc31db9699a3a78085147c777424e8863454d0cc5b42ba33` | yes |
| crates/rah-tools/src/repository_git_layout.rs | `a22c5436d659b639a175039c9a779b7b4cfd0cd52d2b3c273b4612f16cb078da` | yes |
| crates/rah-tools/src/repository_observer.rs | `934eea663afba3c7656c1b51e3f587714ce00766619aab5f6b7604b0604a1e0a` | yes |

Closure git diff --check passed. git status --short and git diff --stat were
recorded after restoration. Only this new report and a reference-only append to
Task 504A were newly changed by Task 504B. No diagnostic source remains.

ADR 0033 is unchanged; no decision or authority impact is established. Static
HostInvocationKind inventory remains exactly 11; no executable inventory or
live Desktop validation was performed. No new Tool, dependency edge, permission,
authority, provider or bypass was introduced.

The optional docs-only commit was not created. No staging, commit, push, CI,
tag, release or version bump occurred. No remote state changed; no Task 504 CI
exists. Final HEAD remains the starting checkpoint. Final worktree is dirty and
uncommitted with Task 504 WIP and diagnostic documentation preserved.
Suggested next task is evidence capture only if separately authorized with a
new bounded scope; migration validation and correction remain stopped.

## Task 504C reference (2026-10-03)

The separately authorized [Task 504C intermittent-state audit and resume gate](2026-10-03-task-504c-intermittent-state-audit-and-validation-resume-gate.md)
records its fixture/shared-state audit and G1 resume recommendation. This is a
reference only: Task 504B's bounded matrix, historical failures, unknown causes
and classifications remain unchanged. Task 504C did not resume Task 504 validation.
