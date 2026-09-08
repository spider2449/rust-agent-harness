# Task 251 — Windows Live HostExplicit `repo.patch` Certification

## Status

TASK 251C — TEST/DOCS DISPOSITION — AWAITING EXACT-HEAD CI

## Task 251C checkpoint

- `HEAD == origin/master == 36d4cfa3e11a7c5ad0a37d65112b684ebfbbfe0c`.
- Task 251A CI: `34192808012 PASS`.
- Worktree was clean before the Task 251C correction.

## Starting checkpoint

- Repository root resolved with `git rev-parse --show-toplevel`.
- `HEAD == origin/master == 55616d071b2e46500d929a2f42f6d727753145dc`.
- Task 250 CI: `34187052229 PASS`.
- Worktree was clean before the harness change.
- Workspace target: 13 packages, version `0.20.0`, edition 2024.

## Certification boundary

This task adds one Windows-only ignored test and no production behavior:

```text
connected-current Desktop composition
 -> repo.patch HostExplicit eligibility
 -> production Prepare / exact review / opaque ticket
 -> ticket-only Confirm
 -> retained-preparer revalidation / D2 preflight
 -> HostExplicit Started
 -> authorized_tool_dispatch / ToolRegistry / ADR 0012 repo.patch
 -> strict ChangedVerified result
 -> repository refresh and currentness checks
```

The test never starts a model turn, sends an `AgentRequest` or prompt, or asks
the model to select `repo.patch`. It uses the actual production Desktop
connection and command functions, with a fresh disposable Windows Git
repository as the mutation fixture. It does not use the RAH checkout as the
fixture and does not perform GUI mouse automation.

## Harness scope

Only these files are changed before the harness commit:

- `crates/rah-desktop/src/main.rs`;
- this plan.

The test is `#[cfg(windows)]`, ignored, and named
`windows_live_desktop_hostexplicit_repo_patch`. It creates a committed target
containing `alpha`, `RAH_PATCH_OLD`, and `omega`, plus one unrelated staged
sentinel for reviewed-authorization setup. All evidence is bounded and omits
absolute paths, environment, authentication, tokens, executable paths, and
native identities.

## Required pre-live validation

Run sequentially without the ignored live test:

```text
cargo fmt --check
cargo check --workspace
cargo test -p rah-tools repository_worktree_patch -- --nocapture
cargo test -p rah-tools
cargo test -p rah-desktop
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
node --check crates/rah-desktop/frontend/status.js
node crates/rah-desktop/frontend/status_authority_test.js
cargo build -p rah-desktop --release
git diff --check
cargo metadata --no-deps --format-version 1
```

Metadata must report 13 packages, version `0.20.0`, edition 2024, and no
Cargo manifest or lockfile diff.

## Harness commit and CI

Commit message:

```text
test: add Windows HostExplicit patch live gate
```

Before push, require the starting `origin/master` checkpoint above, push
`master`, and wait for successful exact-head CI. No ignored live test is
allowed before that CI result.

## Certified live command

After harness exact-head CI passes, run exactly:

```powershell
& .\scripts\codex-live-gate.ps1 `
  -Version '0.149.0' `
  -ExpectedSha256 '14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00' `
  -Model 'gpt-5.6-terra' `
  -ReasoningEffort 'medium' `
  -Command {
      cargo test -p rah-desktop tests::windows_live_desktop_hostexplicit_repo_patch -- --ignored --exact --nocapture
  }
```

The live gate must prove zero model requests and model Tool lifecycle, zero
Prepare Tool execution and replacement attempts, one Confirm Tool execution
and native replacement attempt, exact review and postimage, privacy of
Prepared/Started/terminal HostActivity, reviewed-authorization invalidation,
repository refresh, unchanged Git/index/HEAD/refs/currentness planes, zero
MCP and Process Plugin providers, and an Idle coordinator. No retry, second
Confirm, rollback, restoration, or evidence-hiding cleanup is permitted.

## Live result record

Task 251 remains not certified. The preserved Attempt 2 fixture remains
untouched.

### Attempt 1

- POST-START live attempt was blocked before effect by the Prepare reservation
  defect.
- Ticket, HostExplicit Started, Tool execution, and native replacement were
  all zero.

### Task 251A

- Corrected the production coordinator reservation behavior.
- Commit: `36d4cfa3e11a7c5ad0a37d65112b684ebfbbfe0c`.
- CI: `34192808012 PASS`.

### Attempt 2

- POST-START attempt: Prepare had zero effect; HostExplicit Started occurred
  once; ToolCompleted occurred once; the result was `ChangedVerified`; one
  native replacement occurred; and the exact target postimage was observed.
- Protected Git state was unchanged, including the staged sentinel, HEAD,
  branch, and refs.
- No retry, second Confirm, rollback, or restoration occurred.
- The attempt failed only at the final reviewed/currentness assertion, and the
  disposable fixture was preserved.

### Disposition

Source audit determined that the test assertion was over-strict. The old
one-shot authorization must be invalidated, but the mandatory repository
refresh may create a fresh `ReadyToAuthorize` staged review from the unchanged
sentinel. The corrected invariant requires `ConnectedCurrent`, a fresh
`ReadyToAuthorize` review with no pending underlying authorization, unchanged
current generations and persistence namespace, Idle coordinator and chat,
and zero providers. Freshness is proven by requiring a new review selector,
current workflow review and opaque commit review, with the new review tied to
the post-refresh observation generation; no additional refresh is performed.

Task 251 remains NOT CERTIFIED pending one separately authorized fresh attempt.
Task 251D — Final Fresh Windows HostExplicit `repo.patch` Live Attempt — is not
started automatically.

After a successful fresh run, record the harness commit, exact-head CI,
Windows/Git/Codex baseline, exact command, bounded Prepare and Confirm
evidence, pre/post SHA-256 values, `ChangedVerified`, authorization
invalidation, currentness/non-effects, activity privacy, provider absence,
and all-zero model lifecycle markers here. Then update only this plan, commit:

```text
docs: record Windows HostExplicit patch live pass
```

Push and require final exact-head master/push CI. Task 251 is complete only
after that CI passes. Task 252 is not started automatically.

## Nonclaims

This task does not certify GUI mouse-click automation, model-selected
`repo.patch`, other HostExplicit authoring Tools, external-provider
HostExplicit, or Linux/macOS live behavior.
