# RAH v0.29 Active Repository Close Live Certification

**Verdict: PASS WITH EXPLICIT NONCLAIMS**

The Windows production Desktop backend Close gate passed on the exact certified
source below and emitted `RAH_V029_ACTIVE_REPOSITORY_CLOSE_LIVE_OK`. GUI
automation, model-turn ownership, and a live `HostRunning` owner were not
exercised. Those claims remain outside this live result; deterministic Task 351
coverage remains the evidence for the model-turn and running-owner boundaries.

## Source and environment

- Production behavior base: `99f44aebb89fb697573f16f40a849290d5a9cf09`.
- Starting Task 352 checkpoint: `edc4e95676df63be60a123cfadf3f771e3a47110`.
- Certified source SHA: `c9cff2afac40f9d518310da38ba4c9e158af1641`.
- Certified-source exact-head CI: [run 35212728847 — PASS](https://github.com/spider2449/rust-agent-harness/actions/runs/35212728847).
- The Task 352 source change is confined to the ignored Windows certification
  harness in `crates/rah-desktop/src/main.rs`. No production, frontend,
  manifest, dependency, ADR, persistence-schema, or release-history change was
  included.
- Windows: Windows 10 Professional, build 19045, x64.
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`; Cargo:
  `cargo 1.96.0 (30a34c682 2026-05-25)`.
- Git: `2.54.0.windows.1`.
- Desktop package: `0.28.0`; workspace: 13 packages, all `0.28.0`, edition
  2024; HostExplicit count: exactly 11.
- Ambient PATH Codex: `codex-cli 0.154.0`. It was not substituted for the
  certified runtime. The separately resolved certified Codex baseline was
  available and selected from `CertifiedBaseline`: `codex-cli 0.149.0`,
  SHA-256 `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

## Fixture and measurement

The harness created two disposable Windows Git repositories, A and B, under a
unique temporary certification root outside the development checkout. Each
had a committed baseline, sentinel, staged change, and worktree change. Their
pre-Close captures included `HEAD`, porcelain status, refs, `.git/index` hash,
sentinel and controlled-file hashes, directory existence, and `.git` form.
The temporary root was removed after certification; its private path is not
part of this document or the Close presentation.

Commit-review setup refreshed Git's index cache-tree extension before the
measured P1 Close interval. The harness recorded
`TASK352_COMMIT_REVIEW_INDEX_CHANGED=true` and
`TASK352_PRE_CLOSE_A_INDEX_CHANGED_DURING_SETUP=true`; this setup change was
captured before Close. Each Close and rejected-Close interval compared against
its own immediate pre-operation Git snapshot. No Close changed the index,
refs, worktree, sentinel, repository directory, or status.

The test used a real Tauri-managed `DesktopAppState` and production admission,
activation, workflow refresh, Stage/Unstage reservation, Commit authorization,
Connect, Disconnect, and `close_repository_transition` paths. It did not
implement Close in a fixture. Git observations used the selected native Git
executable against the disposable repositories; no remote Git operation was
issued.

## Close results

### P1 — close A while disconnected

Fresh state admitted A and B, activated A, refreshed real repository workflow,
and prepared safe no-effect Stage/Unstage reservations and a pending reviewed
Commit authorization. Membership IDs and order were recorded. Close succeeded
with the runtime/provider disconnected.

- A and B remained admitted with the same IDs and order; membership generation
  did not change. Both were inactive after Close.
- `DesktopRepository` became `None`; active member became `None`; repository
  generation advanced exactly once, `1 → 2`.
- Workflow actions/review, Commit capability/authorization, HostPrepared
  ticket, and in-memory executable conversation context were withdrawn.
  Conversation epoch advanced once and history/context were empty.
- Effective Authority immediately reported no repository, no selected
  repository generation, no repository Tool inventory, and Commit
  `NotApplicable`. Runtime was still `NotConnected`; provider activation was
  absent.
- The retained HostPrepared ticket could not be taken after Close. No Tool
  dispatch or native effect was started.
- Exact Git captures for A and B and the complete host-storage byte snapshot,
  including remembered catalog and completed transcript data, were unchanged
  across Close.

Repeated Close returned bounded `no_active`. Repository and membership
generations, membership, conversation epoch, Git, catalog, and storage bytes
did not change.

### Reactivation and stale A artifacts

Explicit activation retained A's same process-local member ID and unchanged
membership generation, created a fresh `DesktopRepository`, advanced the
repository generation `2 → 3`, and left the runtime disconnected. Workflow
state had to be freshly observed. Pre-Close Stage, Unstage, Commit-review, and
HostPrepared identities were rejected as stale/unusable; none was executed.

### P2 — stale A guard after switching to B

With A active at generation 3, the harness captured the A guard and explicitly
activated B. B became current at generation 4. Submitting the old A/generation
3 Close request returned bounded `active_changed`. B's repository object,
generation, workflow selectors/review/authorization, Effective Authority, and
both repositories' Git captures were unchanged. No retry or implicit Close of
B occurred.

### P3 — close B normally

Fresh B/current-generation Close succeeded. A and B remained admitted and
inactive, membership generation was unchanged, active member and
`DesktopRepository` were absent, workflow and Commit state were empty, and
Effective Authority had no repository Tools. Repository generation advanced
`4 → 5`.

The measured sequence was `P1: 1 → 2`, A reactivation `2 → 3`, B switch
`3 → 4`, and P3 `4 → 5`.

## Busy owners and connected runtime

- **Stage reservation:** a real A Stage selector acquired the production
  reservation. Close returned `repository_effect_busy`; A and its generation
  remained current, reservation token remained owned, and `.git/index` stayed
  unchanged. The existing owner completion path released the reservation
  without a native Stage operation.
- **Unstage reservation:** separately repeated with a real Unstage selector.
  Close returned `repository_effect_busy`, preserved the owner and A, and did
  not change `.git/index` or execute native Unstage.
- **Commit:** the real workflow reached a no-effect reviewed/pending Commit
  authorization. Close withdrew it; `HEAD`, refs, index, and worktree stayed
  unchanged. No Commit was executed. Old review/authorization remained stale
  after reactivation.
- **HostPrepared:** the production coordinator was placed in a safe prepared
  state through its test-only `PreparedHostInvocation::for_test` seam. Close
  invalidated that state and its ticket was unusable afterward. No Tool
  dispatch or native attempt occurred; a full HostExplicit Prepare/Confirm flow
  was not executed.
- **Connected runtime:** the harness resolved and verified the certified
  Codex 0.149.0 executable and explicitly connected through the Desktop
  lifecycle. Close returned `connected_or_runtime_busy`, preserving active A,
  its repository/generation, current authority/tool inventory, connected
  runtime, and absent provider activation. Close did not Disconnect or shut
  down the provider. An explicit Disconnect then succeeded; the attributable
  certified Codex app-server child was reaped, state reached `NotConnected`,
  provider activation remained absent, and no reconnect occurred. Close then
  succeeded with A still admitted.

The connected snapshot contained 14 effective Tools, of which the exact 11
HostExplicit tools were eligible.

## Conversation, remembered state, privacy, and restart

The harness persisted a completed transcript pair through the production
store API. Conversation presentation and the complete host-storage byte
snapshot were equal before and after Close. After a fresh Desktop startup, the
neutral transcript presentation was empty and not resumable; the repository-
scoped transcript was not automatically selected or resumed. A's in-memory
executable conversation context was absent. Remembered catalog bytes,
candidate ID/label/order/hint presentation, and descriptive availability were
unchanged. Startup restored no membership, active repository, repository
generation authority, workflow, Stage/Unstage state, Commit capability,
HostExplicit preparation, provider activation, runtime binding, or executable
conversation context.

Close result/status, bounded Close errors, serialized membership, and
Effective Authority were checked against a distinctive private-path sentinel,
canonical/native repository paths, `.git`, Git executable details, filesystem
identity, generation-bearing error text, HostExplicit ticket, and provider
handle strings. None appeared on these Close-specific surfaces or in the
captured certification output. Close status was the bounded
`Repository closed. No repository is active.` No path or authority data was
added to activity.

After all checks, the certification-owned Codex process was reaped and the
unique temporary root was removed. No global process-name kill was used.

Post-run inspection also found seven storage-only roots left by earlier,
non-certifying harness attempts; their repository trees were already gone and
their owning test PIDs were no longer running. The attempted recursive removal
of those exact prior-run roots was rejected by the execution policy. They were
left untouched, so cleanup of those seven historical storage directories is
still outstanding. This does not change the final-source live result or
marker, and is not a claim that all artifacts from earlier failed attempts
were removed.

## Frontend carry-forward and validation

The actual Node VM tests passed, including the captured A/17 confirmation
payload remaining A/17 after the displayed state changed to B/18. They also
checked Close does not implicitly Disconnect, retry, activate, remove, or
forget, and that a failed Effective Authority refresh clears the old Close
generation. These are frontend behavior tests, not GUI automation.

Validation on the certified source passed:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace -- --test-threads=1` — all packages passed; Desktop:
  310 passed, 15 ignored.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo build -p rah-desktop --release`
- `cargo metadata --no-deps --format-version 1` — 13 packages, all
  `0.28.0`, edition 2024.
- `node --check crates/rah-desktop/frontend/status.js`
- `node crates/rah-desktop/frontend/repository_membership_test.js`
- `node crates/rah-desktop/frontend/status_authority_test.js`
- `node crates/rah-desktop/frontend/remembered_workspace_test.js`
- `node crates/rah-desktop/tauri_permission_test.js`
- `git diff --check`

## External-action accounting

| Action | Count |
|---|---:|
| Model requests / model Tool requests | 0 / 0 |
| MCP / Process Plugin activations | 0 / 0 |
| Git mutations caused by Close | 0 |
| Network Git operations issued by the certification | 0 |
| Automatic commits / activations / provider reconnects | 0 / 0 / 0 |
| Model-selected Close actions | 0 |

## Explicit nonclaims

- GUI automation was **NOT EXECUTED**.
- A live model-turn/chat-owner busy case was not executed; the certification
  made no model request. Task 351 deterministic lifecycle coverage remains the
  evidence for this boundary.
- A live `HostRunning` owner was not created. No external effect was started
  solely to hold it; deterministic Task 351 lifecycle coverage remains the
  evidence for the running-owner boundary.
- The HostPrepared check used a test-only coordinator reservation seam, not a
  full HostExplicit Prepare/Confirm flow.
- This gate does not establish model-selected dynamic Tool dispatch, OS
  sandboxing, network isolation, rollback/replay/compensation, race-free TOCTOU,
  cross-platform live parity, linked-worktree Close behavior, or persistent
  executable repository membership.
- Close did not implicitly Disconnect, and restart did not automatically
  Resume the transcript; neither observation claims broader lifecycle or
  persistence guarantees.

The Task 351 deterministic source audit remains
`docs/plans/2026-09-17-task-351-active-repository-close-security-audit.md`.
No release preparation was performed as part of Task 352.
