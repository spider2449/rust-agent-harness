# Task 498A — Neutral runtime error envelope validation recovery

## Task 498B infrastructure disposition

The historical Task 498A infrastructure failure below remains preserved.
Authorized Task 498B rebuilt the unchanged source in a previously nonexistent
`F:/temp/rah-task498-target-isolated`, with no copied cache or toolchain change.
Fresh Desktop test linking passed; I1 — STALE/CORRUPT BUILD ARTIFACT CONFIRMED.
The canonical Desktop gate then passed 325/0/20, exit 0. Fresh serial workspace
fmt/check/test/Clippy/diff gates passed: 1,020 tests passed, 0 failed, 24 ignored.
Frontend syntax and four static suites, Tauri inventory (47 commands across
all inventories), metadata (13 packages) and static/executable HostExplicit
inventory (exactly 11) passed. All 17 frozen WIP file hashes remained unchanged
through validation. The original target and full LNK1223 log remain preserved.

Task 498B classification is A — TRANSIENT BUILD ARTIFACT CORRUPTION CLEARED;
TASK 498 VALIDATION COMPLETED. This authorizes the intended Task 498 commit and
normal two-remote publication with exact-head CI; publication results belong
in the final response. See the Task 498B audit report for exact artifact,
toolchain, command, hash and evidence paths. No authority expansion, product
source edit, live provider validation or Task 499 work occurred in recovery.

## Checkpoint and preserved evidence

Starting HEAD: `5954189bf721527461efa673b7838d51cf5a20f3`, exactly the supplied
authoritative master. Task 497 CI `37015819945` PASS is the supplied clean
baseline. Existing worktree `F:/temp/rah-task498`, branch `task-498-neutral-errors`,
was reused without reset, stash, discard, recreation, or modification of Task 495.

Initial WIP audit: modified tracked files were protocol/src/lib.rs,
runtime/src/lib.rs, runtime-codex/src/connection.rs and runtime-codex/src/errors.rs.
Untracked files were protocol/src/runtime_diagnostic.rs, runtime/src/failure.rs,
and the Task 498 implementation report (all crate paths under crates/).
No merge conflicts or contradictory design were present. The Arc-backed source
and closed diagnostic draft followed Task 497, but runtime.rs still consumed old
Fault fields and flattened errors; no propagation or source tests were complete.
The audit retained and completed this draft rather than replacing it.

Historical Task 498 remains D: the baseline had 102 passed, 0 failed, 1 ignored
before rustdoc observed edited sources with older compiled dependencies and
failed doctest compilation. That mixed-input run is invalid as a clean baseline,
not evidence of master, adapter, or envelope failure. Original log remains
`F:/temp/rah-task498-codex-baseline.log`. No retrospective baseline reproduction.

## Completed design and boundaries

ADR 0032 records the bounded Task 497 public-boundary decision. RuntimeFailure
retains private sanitized RuntimeDiagnostic and optional Arc of an owned,
Send + Sync + static Error. Adapter conversion always retains the original.
Error::source returns the inner Error reference, not Arc; AgentError's source
returns the envelope. Clone shares the source without requiring provider Clone.
Whole-error equality is removed; generic tests use neutral variant/field checks.
AgentRuntime signatures and object safety are unchanged; no new dependency edge.

RuntimeEvent atomically owns the local envelope and safe protocol projection.
AgentHandle retains its local stream, with explicit into_runtime_events and
compatible into_events projection. Codex start/resume/cancel, connection fault
fanout, pending requests and active/passive turn streams preserve typed sources.
Provider terminal messages use adapter-local TurnFailed; malformed protocol
data uses the original ProtocolViolation. Channel errors retain their real local
typed source. Explicit shutdown propagates transport failure; Drop stays best
effort. MinimalTestRuntime preserves model/Tool sources and existing terminal
codes. rah-session storage is not a runtime carrier and needs no changes.

Only diagnostics/protocol events have serde; envelopes/local events do not.
Diagnostic fields are closed operation/kind plus optional numeric RPC code.
Default envelope Debug/Display never formats the source. No provider type enters
generic runtime. Desktop's existing into_events call is the explicit frontend
projection; its closed frontend error codes and serialized schema are unchanged.
No live provider, model/catalog/version admission, preflight, Task 495 or Task 499
work is performed. Authority, ToolRegistry, Trusted Profiles, remembered workspace
authority, permissions, leases and mutation uncertainty are unchanged.

## Validation sequencing and results

Initial process census found no cargo, rustc, rustdoc or related test executable.
No process was killed. Source edits and formatting finish before diff --check and
a fresh census. Source is frozen throughout focused validation. Commands run
serially with target `F:/temp/rah-task498-target`, logs outside the repository.
Any failure ends the sequence and is recorded only after command termination.

First stable-source attempt: Phase A `cargo test -p rah-runtime --test
runtime_failure` passed 3/0/0, exit 0. Phase B `cargo test -p rah-runtime-codex`
fully exited 101 with E0106 at runtime_tests.rs:497: the new test-only source
helper lacked a named reference lifetime. No semantic tests ran. Exact logs:
`F:/temp/rah-task498a-phase-a.log`, `F:/temp/rah-task498a-phase-b.log`.
Source remained stable for both commands. Validation stopped; no immediate
rerun or edit while Cargo remained active. After full exit and a fresh empty
process census, the test helper received the compiler-suggested named lifetime.
This is a test compilation correction, not an environmental fault or proof of a
semantic envelope regression. The focused sequence restarts after a new freeze.

Results and final classification: pending corrected stable-source validation.

Second stable-source sequence: Phase A 3/0/0 exit 0; Phase B 91 unit + 6
architecture + 11 live-contract tests passed, 0 failed, 1 ignored; doctests 0/0/0;
complete exit 0. Phase C protocol/runtime 24/0/0 plus empty doctests, exit 0.
fmt --check passed. cargo check --workspace fully exited 101 with E0004 at
Desktop main.rs:1914: existing exhaustive frontend_error mapping lacked the new
SharedFailure and TurnFailed cases. Log `F:/temp/rah-task498a-check.log`.
Source was stable throughout. Full validation stopped; after command exit and
an empty process census, both variants were mapped to the existing closed
CodexConnectionFailed code. No raw text or source is emitted. This completes
the downstream compile migration, not a semantic acceptance change.
Desktop is now a directly affected package and gets canonical focused coverage.

## Final stable-source sequence and stop

Final source freeze followed cargo fmt, git diff --check and an empty relevant
process census. No source or documentation edits occurred while validation ran.

| Phase | Exact command | Actual result |
| --- | --- | --- |
| A | cargo test -p rah-runtime --test runtime_failure | 3 passed, 0 failed, 0 ignored; exit 0 |
| B | cargo test -p rah-runtime-codex | 91 unit + 6 architecture + 11 live-contract passed; total 108 passed, 0 failed, 1 ignored; doctests 0/0/0; complete exit 0 |
| C generic | cargo test -p rah-protocol -p rah-runtime | 24 passed, 0 failed, 0 ignored; both doctest suites 0/0/0; exit 0 |
| C Desktop | powershell -NoProfile -File scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:\temp\rah-task498-target -OutputDirectory F:\temp\rah-task498a-desktop-focused | helper build exit 0; Desktop Cargo exit 101 during linking; no Desktop tests executed; gate TEST_FAILURE, outer PowerShell exit 1 |

Final A/B/C generic logs are `F:/temp/rah-task498a-final-phase-a.log`,
`F:/temp/rah-task498a-final-phase-b.log`, and
`F:/temp/rah-task498a-final-phase-c-generic.log`.
Canonical Desktop evidence directory:
`F:/temp/rah-task498a-desktop-focused/20261002-222416-489-6b2d92c154f140788aba957bebed4716`.
Its status.json, helpers.stderr.log, desktop.stderr.log and desktop.stdout.log
are preserved. Elapsed 181.875 seconds; watchdog did not fire.

Exact underlying failure (desktop.stderr.log):

```text
error: linking with `link.exe` failed: exit code: 1223
libtauri-1f50c6f83fc386ac.rlib(tauri-1f50c6f83fc386ac.tauri.3773942bfd718f42-cgu.12.rcgu.o) : fatal error LNK1223: invalid or corrupt file: file contains invalid .pdata contributions
error: could not compile `rah-desktop` (bin "rah-desktop" test) due to 1 previous error
```

The linker is MSVC 14.51.36231 (Visual Studio 18 BuildTools); the named failing
object is in the Tauri dependency library, not a RAH semantic assertion. The
precise cause of the invalid contributions is not established. This is a
toolchain/artifact validation infrastructure failure, not evidence of runtime
envelope behavior failure. No claims about resource pressure, transient failure,
or a known Tauri/compiler defect are inferred. Cargo fully terminated and a
fresh census found no cargo/rustc/rustdoc/related test processes. No retry,
clean, target deletion, toolchain change, or source patch followed this failure.

## Full validation, inventory and authority limits

Full workspace validation was not completed. Earlier fmt --check passed,
exit 0, before the final Desktop mapping/test additions. Earlier workspace check
failed E0004 as recorded above; the corrected workspace check was NOT RUN.
Workspace tests and Clippy were NOT RUN. Post-full canonical Desktop gate,
frontend/static tests, executable HostExplicit inventory, Tauri permission
inventory and cargo metadata sanity were NOT RUN because focused validation
stopped. No workspace or Desktop pass count is claimed.

Fresh source inventory read HostInvocationKind's eleven variants (FsRead,
RepoFileInfo, RepoStatus, RepoDiff, RepoDiffStaged, RepoCreateBranch, RepoPatch,
RepoEditFiles, RepoCreateFile, RepoDeleteFile, RepoRenameFile). Static
HostExplicit count remains 11; executable inventory remains UNVERIFIED.
Diff scope contains no repository authority, ToolRegistry authorization, Trusted
Profile, remembered workspace authority, permission, lease or mutation
uncertainty changes. No manifests, lockfile, dependency edges, runtime admission,
version, tags or releases changed. ADR 0032 documents the existing selected
neutral envelope decision; accepted authority ADRs remain unchanged.

Source/downcast/clone/redaction tests passed through the actual local runtime
path. Full integration validation is incomplete, so the implementation is not
certified for publication. No observed source serialization leak or required
AgentRuntime trait change was proven.

## Final classification and Git state

**D — VALIDATION INFRASTRUCTURE FAILURE**

HEAD remains `5954189bf721527461efa673b7838d51cf5a20f3`. No Task 498/498A commit,
GitHub push, internal mirror push or new exact-head CI exists. Both push results
and new CI are NOT RUN. Supplied Task 497 CI PASS remains the clean baseline,
not validation of this dirty implementation. WIP remains intentionally dirty,
unstaged: 11 modified tracked files and 6 untracked intended source/test/ADR/report
files. Final git diff --check passed; no conflict entries. Task 495 is untouched
and Task 499 has not started. Further validation recovery requires separate
re-entry; do not infer permission to retry this stopped focused gate.
