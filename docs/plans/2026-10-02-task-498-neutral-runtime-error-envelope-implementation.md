# Task 498 — Neutral runtime error envelope implementation (stopped)

## Task 498A recovery annotation

The historical **D — DETERMINISTIC VALIDATION FAILED** result below is preserved.
The initial command was still active when source edits began: 102 passed,
0 failed, 1 ignored across executable tests, followed by doctest compilation
against mixed source/dependency state. This is an invalid clean baseline and
does not establish a master, adapter, or envelope product defect.

Task 498A explicitly supersedes the remaining-work suggestion to recreate a
baseline in this dirty worktree. Its authoritative clean baseline is
`5954189bf721527461efa673b7838d51cf5a20f3` with supplied Task 497 exact-head CI
`37015819945` PASS. See the separate Task 498A recovery report. Nothing in that
recovery rewrites this historical result or authorizes Task 495/499 work.

## Checkpoint and disposition

Starting and final HEAD: `5954189bf721527461efa673b7838d51cf5a20f3`.
Dedicated worktree: `F:/temp/rah-task498`, branch `task-498-neutral-errors`.
HEAD equality and clean status were verified before source edits. The older
primary checkout was not advanced. Task 495 was not edited, reset, cleaned,
stashed, rebased, committed, or validated; no Task 495 files were copied.

**D — DETERMINISTIC VALIDATION FAILED**

No commit, push, tag, release, version change, or exact-head CI was performed.
The partial patch is intentionally preserved, unvalidated and incomplete.
Do not use this worktree as a validated implementation or resume Task 495.

## Task 497 design and partial source changes

Task 497 selected an Arc-backed neutral envelope, standard Error source
recovery, process-local event ownership, and explicit sanitized projection.
README, architecture guardrails, runtime architecture, security material,
ADRs 0001/0002/0005/0030 and the Task 497 report were consulted.

The draft adds protocol `RuntimeDiagnostic`, `RuntimeOperation`, and
`RuntimeFailureKind` with closed fields and optional numeric RPC code.
No HTTP metadata or provider-specific diagnostic variants were added.
The draft adds runtime `RuntimeFailure` with private diagnostic and
`Option<Arc<dyn Error + Send + Sync>>`; adapter construction always supplies
a source. Its Error implementation returns the inner error reference,
not the Arc wrapper. Display and Debug format only the neutral diagnostic.
Clone shares source allocation without requiring the provider error to Clone.
These properties have not been exercised by deterministic tests.

The draft adds `RuntimeEvent` and `RuntimeEventStream`. AgentHandle retains
the local stream, offers `into_runtime_events()`, and keeps `into_events()`
as an explicit AgentEvent projection. AgentRuntime method signatures remain
unchanged. Existing legacy AgentEvent constructors remain available. No
serde implementation was added to the local envelope/event. Diagnostics
alone are serializable; no source Debug/Display projection is intended.
Serialization separation and downcasting remain UNVERIFIED.

AgentError gains a source-bearing Failure variant and drops whole-error
Eq/PartialEq. Existing equality tests have not yet been adapted.

The Codex draft adds one adapter-owned `into_runtime_failure` converter and
a SharedFailure carrier for connection fanout, retaining the original error
in the envelope. Connection Fault events and pending request errors share
the envelope; explicit shutdown forwards the transport result. Concrete
CodexRuntime creation/shutdown APIs still return concrete adapter errors.

The propagation migration is NOT complete: runtime.rs still matches the old
Fault fields, agent_start_error still flattens errors, and turn streams still
use their legacy representation. No claim is made that the partial patch
compiles. No model/list, catalog, preflight, UI, runtime admission, certified
lifecycle, runtime factory or second-provider implementation was performed.
The intended ADR for the compatible local-stream API has not been written.

## Validation failure and evidence

Attempted initial authoritative baseline command:

```powershell
$env:CARGO_TARGET_DIR='F:\temp\rah-task498-target'
cargo test -p rah-runtime-codex
```

Full output is preserved at `F:/temp/rah-task498-codex-baseline.log`.
Exit code: **1**.

Observed executable test results:

- unit tests: 85 passed, 0 failed, 1 ignored;
- architecture integration tests: 6 passed, 0 failed;
- live-gate contract integration tests: 11 passed, 0 failed.

The agent edited sources before this command finished. Rustdoc subsequently
read new adapter source against dependencies compiled before those edits.
It failed with E0432/E0425 for new RuntimeDiagnostic/RuntimeFailure types
missing from the earlier compiled dependencies. This is an agent sequencing
error and mixed-input validation failure, NOT evidence of an authoritative
checkpoint product defect. The overall command is not a passing baseline.
The historical Task 495 result is not used as the current baseline.

The first failure was inspected and work stopped under Task 498's stop rule.
No retry, focused implementation tests, full workspace gates, Desktop gate,
frontend/static tests, Tauri inventory gate or metadata validation followed.
No new tests were added. There are no workspace pass counts to report.
No live model/provider call was made. A planned mechanical migration script
outside the repository (`F:/temp/task498-edit.py`) did not execute because
`python` was unavailable; it is not part of the implementation.

Final `git diff --check` passed for the partial patch. Source/test validation
must not be inferred from that whitespace check.

## Desktop, authority, dependencies and remaining work

No Desktop or frontend source was changed. No authority, ToolRegistry,
Trusted Profile, permission, repository lease, remembered workspace,
mutation uncertainty or HostExplicit code was changed. HostExplicit remains
the supplied checkpoint value 11; its executable inventory gate was not run.
RAH version remains 0.33.0, preferred Codex 0.157.1, and v0.34 capability
NONE SELECTED. No Cargo manifests, lockfile, dependency edges or accepted
ADRs were changed. Errors carry diagnostic data, not authority.

Security/redaction guarantees of the draft have not been validated. No
observed IPC source leak is claimed, and classification E is not established.
No unavoidable AgentRuntime trait break or ownership conflict was proven.

Remaining work requires a separately authorized re-entry: preserve this log
and partial patch; establish an isolated, unmodified baseline; review/finish
the compatible stream contract and ADR; complete runtime propagation and
typed/redaction tests; follow the requested focused/full gate order. Task 499
is not authorized by this failed Task 498 result.

## Final Git state

Task 498 HEAD is unchanged. Four tracked source files are modified; two new
source files and this report are untracked. Nothing is staged or committed.
GitHub/internal mirror pushes and new CI are NOT RUN. Worktree is dirty
because failed-task evidence is preserved. Task 495 remains stopped.
