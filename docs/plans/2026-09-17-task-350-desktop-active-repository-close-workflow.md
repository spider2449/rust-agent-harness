# Task 350 — Desktop Active Repository Close Workflow

Status: frontend implementation and local validation complete.

Parent checkpoint: `a6f2610ddbda7ade8e8b987cab2385ed0c15eb0e`.
Backend source: Task 349 implementation at
`4a29ef58533a2e9fcc9e16cb28f47b8e74cdafb7`.
Authoritative lifecycle contract: [Task 348 active repository close contract](2026-09-16-task-348-active-repository-close-contract.md).

## Scope and implementation

Expose the existing `close_repository` host lifecycle command in the admitted
repository panel. The Close button targets `renderedRepositoryMembership`'s
active member, regardless of the member selector's presentation selection.
Before opening confirmation, the frontend captures only
`expectedActiveMemberId` and a positive safe-integer
`expectedRepositoryGeneration` from the rendered membership and supported,
selected Effective Authority snapshot. Confirmation retains that pair in the
transient `pendingRepositoryClose` value. Confirm submits only the captured pair
under the existing `request` parameter; it does not resolve or recapture the
currently active repository.

The Close button requires an active member, a selected supported Effective
Authority snapshot with a positive safe-integer current generation, exact
`not connected` app status, and no visible chat/model turn. Busy or unavailable
states remain disabled with bounded guidance. Close does not call Disconnect,
Activate, Remove, remembered workspace mutations, conversation history
mutation, or HostExplicit cancellation.

Confirmation explains that membership remains admitted but inactive, repository
files and remembered workspaces are untouched, and Disconnect is separate.
Cancel clears pending state and invokes no command. Backend success renders the
returned membership, clears repository snapshot/status/diff/review/action
presentation, shows `Repository closed. No repository is active.`, then refreshes
Effective Authority, app status, and transcript through their existing read
paths. It does not request `repository_snapshot` as the success reset path.
`no_active` and `active_changed` receive bounded messages and current-presentation
refreshes without retrying Close.

The default Tauri capability adds only `allow-close-repository`. The generated
permission is unchanged. No Rust production code, dependency, persistence
schema, authority contract, ADR, or HostExplicit membership changed.

## Deterministic frontend evidence

The repository-membership frontend test executes `status.js` in a small DOM
stub and covers enabled/disabled state, supported and unavailable generations,
active-member targeting with another member selected, capture before
confirmation, and the A/generation-17 to B/generation-18 dialog race. Confirm
still sends A/17 with exactly the two allowed request fields. It also covers
cancel, bounded backend errors, no automatic retry, success membership/status,
repository action/review cleanup, Effective Authority/status/transcript refresh,
and Close-path isolation from Connect, Disconnect, activation, removal,
remembered-workspace changes, and HostExplicit cancellation.

The authority test verifies the current generation remains a displayed value
from Effective Authority and that unsupported or unavailable snapshots cannot
be used for the Close guard. The Tauri permission test requires exactly one
default `allow-close-repository`, exact generated command permission, an exact
default permission list, and no wildcard.

## Validation

- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `node --check crates/rah-desktop/frontend/repository_membership_test.js` — PASS.
- `node crates/rah-desktop/frontend/repository_membership_test.js` — PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` — PASS.
- `node crates/rah-desktop/frontend/remembered_workspace_test.js` — PASS.
- `node crates/rah-desktop/tauri_permission_test.js` — PASS.
- `cargo fmt --check` — PASS.
- `cargo check -p rah-desktop` — PASS.
- `cargo test -p rah-desktop -- --test-threads=1` — PASS, 306 passed and 14
  intentionally ignored.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` — PASS.
- `cargo build -p rah-desktop --release` — PASS.
- Manual GUI flow — NOT EXECUTED; no interactive apps or browsers were
  available in the session. This is not GUI certification.

## Security and scope result

- The stale confirmation test proves Confirm submits the captured A/17 pair
  after the rendered active member and generation change to B/18.
- The request contains no path, display name, selected-member value, runtime,
  provider, permission, or authority metadata. Backend checks remain final.
- Connected, connecting, disconnecting, error, unavailable, and active chat
  states disable Close. No implicit Disconnect, retry, activation, removal,
  forgetting, cancellation, or persistence was added.
- Success removes visible A repository actions and reviews while retaining A in
  membership as inactive; later Activate remains a separate user action.
- Default capability delta is exactly one `allow-close-repository`; wildcard
  permission is absent.
- HostExplicit remains exactly 11. Codex baseline remains `0.149.0`.
- Backend Rust changes: none. Cargo/dependency changes: none.

Task 351 remains the next independent lifecycle/security audit. Task 352 remains
the separate live certification gate.
