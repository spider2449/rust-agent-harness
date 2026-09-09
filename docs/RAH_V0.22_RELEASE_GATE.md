# RAH v0.22.0 Release Gate

## Status

**PREPARED / NOT YET PUBLISHED**

Task 265 prepares the candidate immutable source commit. The `v0.22.0` tag,
GitHub Release, and artifact publication are deliberately not created by this
task. v0.21.0 remains the current immutable published release.

## Release identity and preparation baseline

- Task 264 verdict: **A — v0.22 MILESTONE COMPLETE — RELEASE PREPARATION MAY
  BEGIN**.
- Task 264 exact-head CI: `34297841774` PASS.
- Task 265 preparation baseline:
  `43dce0c505a39975b28f9a0be25aeec4ade1a88b`.
- Workspace version: `0.21.0` -> `0.22.0`.
- Workspace packages: 13; every package uses Rust edition 2024.
- Candidate preparation commit: to be recorded after commit.
- Preparation exact-head CI: to be recorded after the candidate commit is
  pushed and its push-triggered CI passes.
- `v0.22.0` tag object and peeled target: not applicable until publication.
- GitHub Release ID: not applicable until publication.

## Milestone capability

v0.22.0 is **HostExplicit Reviewed Multi-File Edit Authoring**. The connected-
current Desktop human workflow is:

```text
typed bounded multi-file request
 -> zero-effect Prepare
 -> complete backend-derived ordered review
 -> opaque ticket-only Confirm
 -> shared revalidation / D2
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> ToolRegistry
 -> existing repo.edit-files / ADR 0014
 -> strict result classification
 -> descriptive repository refresh
```

This is one confirmed non-atomic change-set, not a transaction or four
independent actions. The bounded contract is 1–4 existing clean HEAD-tracked
regular strict-UTF-8 files; 1–16 exact literal replacements per target; and a
maximum of 64 replacements total. Matching uses the exact original snapshot.
The host derives preimages, postimages, hashes, lengths, and canonical
ascending UTF-8-byte-order path execution. Frontend input order is not
execution order. Complete mutation-relevant review is required;
`review_too_large` fails closed.

## Exact HostExplicit allowlist

Exactly these eight Tools are eligible:

1. `fs.read`
2. `repo.file-info`
3. `repo.status`
4. `repo.diff`
5. `repo.diff-staged`
6. `repo.create-branch`
7. `repo.patch`
8. `repo.edit-files`

The following remain ineligible: `repo.create-file`, `repo.delete-file`,
`repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP Tools,
Process Plugin Tools, and unknown Tools. Eligibility is an exact allowlist,
with no wildcard or category-based rule.

## ADR and authority boundaries

- ADR 0014 `RepositoryMultiFileMutationPolicy` remains the existing
  underlying mutation authority for `repo.edit-files`.
- ADR 0023 defines the capability-specific reviewed HostExplicit workflow; it
  does not replace or widen ADR 0014.
- ADR 0021 remains the general HostExplicit coordinator, currentness,
  provenance, and D2 boundary.
- Model output is never authorization. Frontend controls are presentation and
  typed input only. Permission classification does not create mutation
  authority. Trusted Profile and provider metadata cannot self-enable
  HostExplicit.
- HostExplicit uses `authorized_tool_dispatch` and `ToolRegistry`; it does
  not bypass the existing Tool or policy.
- Stage, Unstage, and reviewed Commit remain separate explicit operations. No
  automatic Stage or Commit is part of this release preparation.

## Security contract

Prepare is zero-effect: zero Tool executions, native attempts, repository
effects, and authority persistence. The opaque authority ticket is process-
local, in-memory, single-use, exact-change/currentness-bound, and valid for an
inclusive five-minute TTL. Confirm and Cancel receive only `{ ticketId }`.

Generic HostExplicit activity uses a separate RAH-generated non-authority
activity ID. The activity ID is not the ticket, is not derived from the ticket,
cannot Confirm, and cannot Cancel. Generic activity excludes the actual ticket
and source-bearing review content, including raw Tool input/output, absolute or
native paths, and source-bearing errors. Task 263 Attempt 1 discovered the
ticket-under-`invocationId` leak before Confirm with zero effect. Attempt 2
corrected the privacy defect and passed on a fresh repository; both attempts
remain part of the evidence chronology.

ADR 0014 result classes remain distinct:

```text
ok
invalid_target
precondition_failed
failed_known_no_effect
partial_effect
uncertain
```

The operation is explicitly non-atomic. `partial_effect` reports only the
verified committed prefix; `uncertain` preserves unknown effect. There is no
retry, replay, prefix continuation, rollback, restore-preimage, compensation,
or transaction claim. Timeout, cancellation, disconnect, crash, or lost
response does not imply rollback. `repo.patch` and `repo.edit-files`
invalidate stale repository-bound reviewed Commit authorization at Started.
Refresh is descriptive only; the narrow empty-index fallback preserves
status/diff observation without fabricating or granting Commit authority.

## Deterministic validation

The Task 265 preparation validation is recorded in the plan and must include:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
node --check crates/rah-desktop/frontend/status.js
node crates/rah-desktop/frontend/status_authority_test.js
cargo build -p rah-desktop --release
cargo test -p rah-tools
cargo test -p rah-desktop
```

Metadata must show exactly 13 workspace packages, all version `0.22.0`, all
edition 2024, and no remaining `0.21.0` workspace package. Cargo.lock may
change only for internal RAH workspace package version bookkeeping. No
external dependency version, source, checksum, edge, feature, addition, or
removal drift is allowed.

## Task 263 Windows evidence carried forward

Task 263 final Attempt 2 is the final connected-current production evidence;
Task 265 does not rerun live certification or submit a model prompt:

- Platform: Windows 11 IoT Enterprise LTSC x64.
- Codex baseline: `0.149.0`.
- Certified SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Model: `gpt-5.6-terra`; reasoning: `medium`.
- Caller order: `d.txt, b.txt, a.txt, c.txt`.
- Backend/effect order: `a.txt, b.txt, c.txt, d.txt`.
- Prepare: 0 Tool executions; 0 native attempts.
- Confirm: 1 Tool execution; native attempts `1/1/1/1`.
- Final generations: `[1, 0, 0, 1]`.
- Coordinator: Idle. Chat: Idle. Model lifecycle: all zero.
- MCP: 0. Process Plugin: 0.
- Marker: `RAH_MULTI_FILE_HOSTEXPLICIT_LIVE_OK`.

This is host-driven connected-current evidence, not a model-selected
HostExplicit execution claim. Unix deterministic/platform-gated testing is
not Windows-equivalent live certification. The live evidence does not claim
atomicity, rollback, race-free TOCTOU, network isolation, OS sandboxing,
generic filesystem write, structural HostExplicit authoring, HostExplicit
`repo.commit`, MCP/Process Plugin HostExplicit, ticket persistence/resume, or
automatic Stage/Commit. Process supervision is not OS sandboxing.

## Release-preparation checklist

- [x] Task 264 milestone audit accepted with Verdict A.
- [x] Starting baseline and clean worktree verified.
- [x] `v0.22.0` local and remote tags absent before preparation.
- [x] GitHub Release `v0.22.0` absent before preparation.
- [x] Workspace bumped to `0.22.0` with edition 2024 unchanged.
- [x] Documentation updated within the authorized preparation scope.
- [x] No Rust source, frontend source, tests, scripts, workflows, ADRs,
      permissions, profiles, providers, or dependency definitions changed.
- [ ] Preparation commit recorded and pushed.
- [ ] Exact-head push CI completed successfully for the preparation commit.
- [ ] `HEAD == origin/master` at the preparation commit and worktree clean.
- [ ] `v0.22.0` tag created by the separate publication task.
- [ ] Tag CI and GitHub Release recorded by the separate publication task.

## Publication boundary

Task 265 stops after exact-head CI for the preparation commit. It does not
create or push `v0.22.0`, create a GitHub Release, publish artifacts, modify
a release commit after publication, or start the publication or cleanup task.

The next task, only after independent verification, is the separate immutable
publication step.

## Current immutable baseline

- v0.21.0 annotated tag object:
  `aa178f05d12d881745932800e8fc40a7431ce7b9`.
- v0.21.0 peeled release target:
  `7faa13a16425cc9d3bdb0dc540923eae8685ba90`.
