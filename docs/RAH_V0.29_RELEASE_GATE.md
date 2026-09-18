# RAH v0.29.0 Release Gate

**Status: READY FOR PUBLICATION — NOT YET RELEASED**

This gate records the accepted v0.29 milestone and release preparation for
**Explicit Active Repository Close**. Task 354 is a separate publication gate.
No v0.29.0 tag or GitHub Release exists as part of Task 353.

## Candidate identity

| Identity | SHA / value |
|---|---|
| Production behavior base after Task 351 | `99f44aebb89fb697573f16f40a849290d5a9cf09` |
| Windows-certified source after Task 352 | `c9cff2afac40f9d518310da38ba4c9e158af1641` |
| Task 352 certification documentation head | `b51d85fcfa7fcca4ee81b4bcd73af391ebc8ad2e` |
| Task 353 release-preparation source | The full SHA of the commit containing this gate and the Task 353 plan; it is established by Git after that commit and is not self-embedded. Task 354 must recheck and use that exact commit. |

Task 353 starts at `b51d85fcfa7fcca4ee81b4bcd73af391ebc8ad2e`, with
`HEAD == origin/master` and a clean worktree. The v0.28.0 immutable published
release remains unchanged.

## Milestone audit: Tasks 347–352

| Task | Accepted outcome | Audit record |
|---|---|---|
| 347 | Scope decision selected explicit active-repository Close. Authority delta is `NONE`; no ADR was needed. | [Scope and authority roadmap](plans/2026-09-16-task-347-v0.29-scope-and-authority-roadmap.md), exact-head CI `35099997493` PASS. |
| 348 | Froze the Close lifecycle, currentness, invalidation, persistence, failure, and evidence contract without changing an ADR. | [Close contract](plans/2026-09-16-task-348-active-repository-close-contract.md). |
| 349 | Added guarded active-to-none withdrawal and final workflow-publication currentness checks. Its exact-head full-workspace CI `35177852723` passed. | [Withdrawal foundation](plans/2026-09-17-task-349-zero-active-repository-withdrawal-foundation.md). |
| 350 | Added explicit Desktop confirmation capturing active member and repository generation, the exact frontend permission, and no implicit Disconnect. Exact-head CI `35180299179` passed. | [Desktop workflow](plans/2026-09-17-task-350-desktop-active-repository-close-workflow.md). |
| 351 | Independent audit verdict **PASS WITH NARROW HARDENING**. Production behavior base: `99f44aebb89fb697573f16f40a849290d5a9cf09`. Audit/docs head `edc4e95676df63be60a123cfadf3f771e3a47110`; exact-head CI `35184979132` passed. | [Security audit](plans/2026-09-17-task-351-active-repository-close-security-audit.md). |
| 352 | Windows live verdict **PASS WITH EXPLICIT NONCLAIMS** on exact source `c9cff2afac40f9d518310da38ba4c9e158af1641`; marker `RAH_V029_ACTIVE_REPOSITORY_CLOSE_LIVE_OK`. | [Live certification](RAH_V0.29_LIVE_CERTIFICATION.md) and [Task 352 record](plans/2026-09-17-task-352-v0.29-active-repository-close-live-certification.md). Source CI `35212728847` PASS; certification-docs CI `35214134363` PASS. |

The sequence is complete and has no unresolved mandatory blocker. Task 347's
accepted product decision is not reopened here. Intermediate Task 352 harness
iterations were non-certifying harness attempts and emitted no marker; they
are not failed production builds or failed product certification.

## Release claim and transition

RAH Desktop can explicitly withdraw the currently active repository into a
valid zero-active state while retaining that repository as an admitted
inactive member. Later use requires separate explicit activation.

```text
members = [A, B?]
active = A

explicit Close A

members unchanged
A retained and inactive
active = none
DesktopRepository = none
active repository executable authority = absent
```

Close is distinct from Disconnect, Switch, Remove, Forget, filesystem
deletion, and Git mutation. It requires the existing runtime/provider lifecycle
to be fully disconnected and never performs Disconnect implicitly.

The confirmation captures expected active `RepositoryMemberId` and repository
generation. These are equality/currentness guards, not authority. If A was
confirmed and B becomes active before Confirm reaches the backend, the request
returns `active_changed`, preserves B, and is not reinterpreted as “close the
current repository.”

Membership generation does not change. Successful Close advances repository
generation exactly once with checked arithmetic; rejected or repeated Close
does not churn it. Explicit reactivation creates fresh repository currentness.
Task 352 measured P1 Close A `1 → 2`, Reactivate A `2 → 3`, Switch A to B
`3 → 4`, and P3 Close B `4 → 5`; those values are one live sequence, not
globally fixed generation numbers. Observation and selector/action sequence
advancement is checked so exhaustion does not wrap to stale IDs. Final async
workflow publication rechecks current repository, member, and generation under
lifecycle coordination, preventing stale A work from publishing after Close
or switch.

## Authority and ADR status

**Authority delta: NONE.** Close withdraws existing active repository
authority under ADR 0027. It is not a Tool, HostExplicit action, Codex dynamic
Tool, MCP Tool, Process Plugin Tool, Git authority, or filesystem authority.
HostExplicit remains exactly 11.

ADR 0027 remains authoritative and unchanged. It already permits zero active
repositories, distinguishes descriptive membership from active executable
authority, and requires fresh active-only composition. ADR 0028 remains
authoritative and unchanged; remembered workspaces remain descriptive. No
v0.29 ADR was required.

## Security and lifecycle behavior

- Close fails closed on poisoned or incoherent state. Checked generation
  arithmetic cannot wrap.
- Started or uncertain model, HostExplicit, Stage/Unstage, Commit, or other
  effect owners remain owned and block Close. Close is not cancellation,
  rollback, replay, or compensation.
- Safe prepared state and current reviewed Commit authorization are withdrawn
  as part of successful Close. Old review/currentness remains stale after
  reactivation.
- A Connected runtime rejects Close. Task 352 live-certified Connected →
  rejected Close → explicit Disconnect and attributable app-server reaping →
  NotConnected → explicit Close. No implicit Disconnect or reconnect occurred.
- Effective Authority after Close reports no active/selected repository
  authority, no active repository generation or Tool inventory, no reviewed
  Commit authority, and a disconnected runtime. Admitted membership alone is
  not executable authority.
- Successful Close resets in-memory executable repository conversation
  context but does not delete completed transcript persistence or automatically
  Resume it. Restart does not silently select a repository-scoped transcript.
- Close does not write the remembered workspace catalog. Candidates remain
  descriptive and are not removed or made executable by Close.
- Close performs no worktree write, create/delete/rename, Stage, Unstage,
  Commit, branch/ref/history mutation, `.git` mutation, or network Git. The
  certification measured zero Git mutations in successful and rejected Close
  intervals. Fixture setup before a measured interval is not attributed to
  Close.

Task 351's deterministic audit also covered chat-owner retention through
conversation/transcript terminal publication; poisoned/incoherent Close
handling; checked workflow observation/action counters; real Unstage
reservation ownership; Effective Authority after Close/reactivation; and
Connect ordering. Task 352 live-certified real Stage and Unstage reservations
and a real no-effect Commit review/authorization without executing Commit.

## Task 352 live evidence

- Environment: Windows 10 Professional, build 19045, x64; `rustc 1.96.0`;
  Cargo `1.96.0`; Git `2.54.0.windows.1`.
- Certified runtime: `codex-cli 0.149.0`, SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
  Ambient Codex 0.154.0 was not substituted. The certified Codex runtime was
  used only for connection lifecycle validation, not model inference.
- At certification time the Desktop package was `0.28.0`; 13 workspace
  packages were all `0.28.0`, all edition `2024`; HostExplicit was exactly 11.
- The Task 352 source delta after Task 351 was confined to the Windows
  certification harness in `crates/rah-desktop/src/main.rs`; production and
  frontend behavior were unchanged.
- External actions: model requests `0`; model Tool requests `0`; MCP
  activations `0`; Process Plugin activations `0`; Git mutations caused by
  Close `0`; network Git operations `0`; automatic commits `0`; automatic
  activations `0`; automatic provider reconnects `0`; model-selected Close
  actions `0`.

## Nonclaims

The release preserves these explicit limits:

- GUI automation was **NOT EXECUTED**.
- The live model-turn/chat-owner busy case was **NOT EXECUTED**; Task 351
  deterministic lifecycle evidence remains the basis for that boundary.
- The live HostRunning case was **NOT EXECUTED**; Task 351 deterministic
  lifecycle evidence remains the basis for that boundary.
- Full live HostExplicit Prepare/Confirm was **NOT EXECUTED**. The HostPrepared
  check used a test-only reservation seam and caused no dispatch/native
  attempt.
- Model-selected dynamic Tool dispatch is not established by this gate.
- No OS sandbox, network isolation, rollback/replay/compensation, or race-free
  TOCTOU guarantee is claimed.
- No cross-platform live parity or linked-worktree Close semantics are
  claimed.
- No persistent executable repository membership is claimed.
- No automatic transcript Resume or implicit Disconnect is claimed.

## Historical certification-harness cleanup limitation

The successful Task 352 certification reaped its attributable Codex child,
removed its unique fixture root, and left the development checkout clean.
Post-run inspection identified seven storage-only roots from earlier
non-certifying harness attempts. Their repository fixture trees were already
absent and owning test PIDs were no longer running. Execution policy rejected
recursive deletion of those exact roots; no alternate deletion route was
attempted. Those seven directories remained. This is a non-blocking historical
certification-harness cleanup limitation, not a live repository fixture or a
running-process leak. Task 353 does not broaden into cleanup work.

## Version and release-preparation validation

Task 353 changes all 13 workspace packages from `0.28.0` to `0.29.0`; edition
remains `2024`. Cargo.lock is expected to change only the 13 internal RAH
package version records. No external dependency, checksum, source, or graph
change is allowed.

Required Task 353 checks:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
cargo build -p rah-desktop --release
node --check crates/rah-desktop/frontend/status.js
node crates/rah-desktop/frontend/repository_membership_test.js
node crates/rah-desktop/frontend/status_authority_test.js
node crates/rah-desktop/frontend/remembered_workspace_test.js
node crates/rah-desktop/tauri_permission_test.js
```

The local validation results are recorded in
[the Task 353 preparation plan](plans/2026-09-18-v0.29-release-preparation.md).
The release build must identify `rah-desktop v0.29.0`. Carry-forward of Task
352 certification is valid only because Task 353 changes version metadata and
documentation, with no Rust behavior, frontend behavior, tests, or permissions
change.

## Exact-head and publication gates

After committing Task 353, push `master`, then require all of the following
before Task 354 begins:

1. `HEAD` equals `origin/master` at the exact Task 353 source SHA.
2. The worktree is clean and the exact changed-file scope is the eight
   preparation paths: `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`,
   `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, this gate, and the Task 353
   plan.
3. Exact-head Task 353 CI completes successfully for that full SHA.
4. No `v0.29.0` tag or GitHub Release has been created during preparation.

Task 353 creates no `v0.29.0` tag, pushes no v0.29.0 tag, creates no GitHub
Release, marks no changelog entry released, invents no publication timestamp
or tag-object SHA, and does not mutate v0.28.0 artifacts. Task 354 will
independently recheck the exact candidate, create the annotated tag at that
exact SHA, require exact tag CI PASS, and then publish `RAH v0.29.0` without a
repository commit.
