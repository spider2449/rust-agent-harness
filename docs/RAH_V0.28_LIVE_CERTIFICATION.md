# RAH v0.28 Inactive Repository Member Removal
# Independent Audit and Windows Live Certification

## Result

Classification: **PASS WITH EXPLICIT NONCLAIMS**.

The v0.28 inactive repository-member removal path was independently audited
against Tasks 339–342 and Task 340's contract. The mandatory host-driven
production-backend certification passed on Windows. No production or frontend
behavior correction was required. A test-only, opt-in certification harness
was added so the live run exercised the existing `DesktopAppState` admission,
activation, lifecycle, and removal functions.

The certified source was:

```text
CERT_SOURCE_SHA=a55347fc1df413fea6ab1647a7d80d88c2c99fe4
origin/master=a55347fc1df413fea6ab1647a7d80d88c2c99fe4
worktree=clean
exact-head CI=35074858762 PASS
```

This is a test-only descendant of the requested checkpoint
`13a155dcaa789218c0779795a4dd12166f66b273`; the requested checkpoint's
Task 342 CI was `35070656244` and passed before this certification harness
was added. The production/frontend behavior under audit was not changed.

The successful mandatory run emitted:

```text
RAH_V028_INACTIVE_MEMBER_REMOVAL_LIVE_OK
```

## Environment

| Item | Recorded value |
| --- | --- |
| OS | Windows 10 Pro, build 19045 (`10.0.19045`), x64 |
| Rust | `rustc 1.96.0 (ac68faa20 2026-05-25)` |
| Cargo | `cargo 1.96.0 (30a34c682 2026-05-25)` |
| Git | `git version 2.54.0.windows.1` |
| Workspace | 13 packages, all `0.27.0`, Rust edition 2024 |
| `rah-desktop` | package/build version `0.27.0`; release binary SHA-256 `ABEFCDC3C631BF14CDDA8FE8611192DAF451E568EBF11D6ACC773620733EC08A` |
| Ambient Codex | `codex-cli 0.154.0`; not used as the certification baseline |
| Certified Codex baseline | `0.149.0`, verified at `C:\Users\spider.tp\AppData\Local\codex-baselines\0.149.0\codex.exe` |
| HostExplicit | exactly 11 |
| Published release | RAH `v0.27.0` remains unchanged |

GUI automation was not available: the computer-control surface reported no
native applications. Accordingly, this is **host-driven production-backend
certification**, not GUI certification.

## Independent audit matrix

| Contract invariant | Evidence and verdict |
| --- | --- |
| Request to removal | The frontend carries only the opaque `memberId`; the backend parses it before acquiring coordination locks and performs currentness checks before mutation. PASS. |
| Selector closure | `RepositoryMemberId` is the only bounded process-local selector. Strict malformed, zero, overflow, leading-zero, and trailing-input forms fail before mutation. No path, label, remembered candidate, Git identity, filesystem identity, Tool identity, or provider identity recovery exists. PASS. |
| Membership primitive | Inactive removal removes exactly one member and advances membership generation exactly once. Active, unknown, and stale targets reject without mutation. Concurrent double removal has one success. PASS. |
| Coordination order | Production removal and activation use `membership_coordination -> lifecycle_coordination -> current state checks -> membership mutation/publication`. No reverse production path was found. PASS. |
| Admission and activation race | Currentness is checked before publication and revalidated while coordinated. The race test proves removal wins without an absent active member; the opposite ordering publishes activation and then rejects removal of the active member. PASS. |
| Busy attribution | Target-bound/current index ownership rejects removal and preserves its owner. An unrelated active-A reservation does not block inactive-B removal. Stale or unattributable ownership fails closed. PASS. |
| No-active fail-closed path | If no active member exists while executable lifecycle state remains, removal returns busy and does not normalize or clean that state. Clean no-active membership permits only inactive membership mutation. PASS. |
| Active-A preservation | Removing B preserves active A, the active repository `Arc`, repository generation, registry definitions/composition identity, provider/connection state, conversation tuple/history, Commit authorization, Stage/Unstage reservation, workflow state, and HostExplicit prepared state. PASS. |
| Active-member rejection | Removing active A is bounded `active` rejection with zero membership-generation, lifecycle, provider, conversation, Commit, index, HostExplicit, Git, and filesystem effect. PASS. |
| Uncertain effects | Started/uncertain target-bound Stage/Unstage and HostExplicit ownership are busy. Removal does not cancel, replay, retry, compensate, roll back, or advance membership generation. PASS. |
| Commit isolation | Inactive-B removal neither revokes A reviewed Commit state nor creates or consumes authorization. B has no independent active executable Commit authority. PASS. |
| Stage/Unstage isolation | Active-A reservation survives B removal; target-B reservation blocks it; removal itself performs no Git-index operation. PASS. |
| HostExplicit isolation | The allowlist remains exactly 11. B removal creates no ticket, consumes no A ticket, clears no A prepared state, and has no ToolRegistry/model invocation route. PASS. |
| Provider/runtime isolation | Inactive membership has no independent provider/runtime composition. The active-only composition remains singular; the live baseline had no connected provider/runtime to tear down. PASS. |
| Conversation isolation | B removal does not reset A conversation, alter its identity or history, or create repository execution authority. PASS. |
| Remembered workspace | Process-local removal does not write `remembered-workspace.json`. Candidate-present and candidate-absent live cases preserve catalog bytes; later admission validates current facts and creates a fresh inactive ID. PASS. |
| Git/filesystem boundary | Removal does not write repository files, `.git`, index, HEAD, refs, history, or worktrees. Live A/B HEAD, index, worktree status, refs, and sentinel bytes were equal before and after. PASS. |
| Frontend permission | Default capability contains exactly `allow-remove-repository-member`; generated permission is command-specific for `remove_repository_member` with no wildcard or scope broadening. PASS. |
| Frontend lifecycle | Remove is enabled only for an inactive selected member, active removal is disabled/noninvoking, confirmation is required, and wording states files and remembered entries remain. PASS. |
| Confirmation race | The backend decides current outcome after confirmation. A changed active target returns `active`; a disappeared target returns `not_found`/stale. No frontend workaround or stale success is used. PASS. |
| Error/privacy | Active, not-found/stale, busy, and invalid-selector errors are bounded. Known frontend errors are mapped to bounded user text; removal output/status contains no path, `.git`, debug tuple, generation tuple, ticket, or provider internals. PASS. |
| ID persistence | Member IDs are process-local routing values only. No member ID is written to local/session storage, IndexedDB, preferences, remembered catalog, transcript DB, or restart state. PASS. |

## Windows live scenarios

The opt-in test was run with
`RAH_RUN_V028_INACTIVE_MEMBER_REMOVAL_LIVE=1` and the exact certified source.
It used two disposable real Git repositories, A and B, with sentinel files.
The RAH development repository was not used as a mutation target.

### P1: core removal, rejection, and re-admission

1. A and B were admitted through production admission. A was explicitly
   activated. Their opaque member IDs were distinct.
2. A's active repository, repository generation, workflow, conversation,
   Commit authorization, active-A index reservation, HostExplicit prepared
   state, and registry definitions were recorded.
3. A synthetic target-bound B index-effect owner was installed through the
   existing lifecycle state for the impossible-by-active-only live case.
   Removing B returned bounded `busy`; B, membership generation, remembered
   catalog bytes, and the owner were unchanged. This is deterministic
   owner-attribution coverage, not a claim that an inactive member can own a
   live provider or active repository composition.
4. The owner was cleared, an unrelated active-A reservation was retained, and
   removing inactive B succeeded. B disappeared, generation advanced exactly
   once, and A's recorded executable state remained unchanged. The active-A
   reservation remained owned.
5. Repeating the old B selector returned bounded stale/not-found with zero
   effect. A malformed selector also returned bounded invalid-selector with
   zero effect.
6. Removing active A returned bounded active rejection. A remained admitted
   and active; membership generation and all recorded lifecycle, provider,
   conversation, Commit, index, HostExplicit, Git, and filesystem state were
   unchanged.
7. B was explicitly re-admitted from its remembered candidate. Its new member
   ID differed from the old ID and it remained inactive. The old selector was
   still invalid.
8. The new B was explicitly activated through the existing activation action,
   then the existing explicit switch action returned to A. This demonstrates
   separation: removal and re-admission do not activate a member and contain
   no remove-to-activate chain.
9. The remembered B candidate was then removed as an explicitly controlled,
   unrelated setup operation. B was admitted by path without a candidate and
   removed process-locally. The durable catalog bytes remained unchanged by
   removal in both candidate-present and candidate-absent cases.

### P2: restart boundary

After P1 state was dropped cleanly, a fresh `DesktopAppState` was created with
the same storage. Before explicit admission it had:

```text
membership count = 0
active member = none
active repository = none
repository ToolRegistry/composition = absent
Commit authority = absent
Stage/Unstage lifecycle state = absent
HostExplicit prepared state = absent
provider/runtime repository binding = absent
executable conversation repository context = absent
```

Descriptive remembered candidates may persist, but process-local membership
and executable authority do not.

## Effect and accounting evidence

The harness compared the real Git state of A and B before and after removal:
HEAD, relevant refs, index state, worktree status, and sentinel contents were
unchanged. No automatic commit, index mutation, filesystem write, repository
file move/delete, network Git operation, or development-repository mutation
occurred. All disposable repositories and temporary storage roots were
removed; no certification-owned child processes remained. No provider child
was started.

```text
model requests              = 0
model Tool requests         = 0
MCP activations             = 0
Process Plugin activations  = 0
network Git operations      = 0
automatic commits           = 0
automatic activations       = 0
```

The explicit host activation and switch in P1 are not automatic activations.

The privacy scan covered removal presentation/result serialization, bounded
error text, frontend status text, and the emitted certification/accounting
surfaces. The distinctive native fixture paths did not appear in removal
result/error/status output. The ordinary repository UI may still display a
configured presentation name; that is separate from removal authority
identity.

## Validation

Before source certification, the following passed:

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
node crates/rah-desktop/frontend/remembered_workspace_test.js
node crates/rah-desktop/frontend/status_authority_test.js
node crates/rah-desktop/tauri_permission_test.js
```

The final-source exact-head CI was run as GitHub Actions CI run
`35074858762` and completed with `success` for the exact source SHA above.
The opt-in live test completed with one passed, zero failed, and zero ignored
for the selected test invocation.

## Nonclaims

This certification does not claim:

- model-selected dynamic Tool dispatch;
- an OS sandbox or network isolation;
- rollback, replay, compensation, or cancellation of uncertain external effects;
- race-free TOCTOU behavior beyond the reviewed currentness and publication
  protocol;
- cross-platform live parity;
- GUI automation or GUI certification;
- cross-process persistent membership or linked-worktree removal semantics.

GUI automation was **NOT EXECUTED**. The certified result is the mandatory
host-driven production-backend result with that explicit GUI nonclaim.
