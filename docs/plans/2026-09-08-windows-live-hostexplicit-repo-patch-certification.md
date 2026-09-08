# Task 251 — Windows Live HostExplicit `repo.patch` Certification

## Status

LIVE PASS — AWAITING FINAL CI

## Task 251C checkpoint

- `HEAD == origin/master == 05e50396ebb46fd80c4b9f6c8f8ab2a3d4f83175`.
- Task 251A CI: `34192808012 PASS`.
- Task 251C: test-only assertion correction.
- Task 251C commit: `05e50396ebb46fd80c4b9f6c8f8ab2a3d4f83175`.
- Task 251C exact-head CI: `34194725872 PASS`.
- Worktree was clean before the final fresh live attempt.

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

The preserved Attempt 2 fixture remains untouched.

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

Task 251D — Final Fresh Windows HostExplicit `repo.patch` Live Attempt — was
authorized and completed below.

### Attempt 3 — final fresh live pass

- Result: PASS. This was exactly one fresh live attempt using a new disposable
  Windows Git repository. The preserved Attempt 2 fixture was not reused,
  deleted, restored, or altered. No retry, replay, second Confirm, rollback,
  or restore-preimage occurred.
- Host: Microsoft Windows 10 `10.0.19045` (build `19045`). Git:
  `2.54.0.windows.1`.
- Codex gate baseline: `codex-cli 0.149.0`, SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
  Model: `gpt-5.6-terra`; reasoning effort: `medium`.
- Exact command:

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

- Prepare: connected-current; `repo.patch` was HostExplicit eligible;
  `repo.edit-files` was not HostExplicit eligible; MCP providers and Process
  Plugin providers were both zero. Exact `RepositoryPatchReview`, opaque
  ticket, private Prepared activity, zero Tool executions, and zero
  replacements were observed. Target, index, HEAD, and refs were unchanged,
  and the old reviewed authorization remained pending before Confirm.
- Confirm: ticket ID only; retained-preparer revalidation succeeded before
  `Started`; D2 preflight succeeded; HostExplicit Started was one; Tool
  execution was one; native replacement was one; terminal ToolCompleted was
  one. Result classification was `ChangedVerified`.
- Target proof: exact preimage SHA-256 was
  `39b37d45697c4371a30248f1e60244dec3f66bca1254babfcbf25ce021c9af7a` and
  exact expected postimage SHA-256 was
  `0991d9f445b98cfd0566f218cb8ef3ee4cbd47e89380f9bc1eb81737fd77387e`.
  The target was the only worktree-content change in the disposable fixture;
  the staged sentinel was unchanged. Semantic index, HEAD OID, branch, and
  refs were unchanged. No patch temporary artifact remained.
- Review authorization: the old pending authorization was cleared
  (`REVIEW_INVALIDATED=1`). Reviewed-commit presentation was
  `ReadyToAuthorize`, while the underlying reviewed-commit control had no
  pending authorization. A fresh staged review existed without an extra
  refresh solely for proof; its selector differed from the original
  pre-patch selector, its repository generation was current, its observation
  generation matched the current workflow observation generation, and a fresh
  `commit_review` existed.
- Currentness and lifecycle: connection remained `ConnectedCurrent`; the
  generation tuple and persistence namespace were unchanged; coordinator and
  chat were Idle. Prepared, Started, and terminal activity exposed no source
  review. MCP providers and Process Plugin providers remained zero.
- Model non-involvement: `runtime.start=0`, `AgentRequest=0`, `prompt=0`,
  `ToolRequested=0`, `ToolStarted=0`, `ToolFinished=0`, and lifecycle `0/0/0`.
- Gate result: `RAH_DESKTOP_PATCH_LIVE_OK`; one test passed, zero failed.

Task 251 is now complete pending the final docs-only commit and exact-head
CI closure below.

After the successful fresh run, update only this plan, then commit:

```text
docs: record Windows HostExplicit patch live pass
```

Push and require final exact-head master/push CI. Task 251 is complete only
after that CI passes. Task 252 is not started automatically.

## Nonclaims

This task does not certify GUI mouse-click automation, model-selected
`repo.patch`, other HostExplicit authoring Tools, external-provider
HostExplicit, or Linux/macOS live behavior.
