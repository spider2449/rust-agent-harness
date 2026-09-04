# Task 195 — Effective Authority Cross-Layer Hardening

Date: 2026-09-04
Status: deterministic hardening complete; awaiting reviewed commit and exact-head CI

## Scope and baselines

This task audited the complete host snapshot, Tauri command, refresh orchestration,
and Effective Authority renderer path. It added no authority, lifecycle control,
provider functionality, persistence, dependency, version, ADR, or release change.

Starting `HEAD` and `origin/master` were both
`a784727055272d2b253e82aeb713c043cff7ba1a`; the worktree was clean. Task 192 was
the authoritative UX/security contract, Task 193 supplied the sanitized backend
DTO and command, and Task 194 supplied the renderer and lifecycle refresh wiring.
The v0.15.0 release commit and tag were not modified.

## Corrective change

`status.js` now treats an unknown `status` value as unsupported in addition to
rejecting unsupported `schemaVersion`. It clears all authority summary,
inventory, unavailable-capability, and advanced elements before showing bounded
unavailable wording. Therefore an unknown backend state cannot leave old or
untrusted inventory visibly presented as Current.

## Audit results

### Schema and state

The backend serializes the Task 193 top-level fields as camelCase:
`schemaVersion`, `status`, `repository`, `connection`, `configured`,
`effectiveTools`, `unavailableCapabilities`, and `reviewedCommit`. The renderer
consumes only those fields and their closed nested fields. Rust tests assert the
complete serialized field set and stable repeated serialization.

All seven backend statuses are mapped. Renderer Current is reachable only from
`snapshot.status === "connected_current"`; connection state, repository identity,
and generations do not independently produce Current. Unknown status and schema
fail closed. Closed enum label fallbacks are bounded `Unknown / unavailable`.

### Lifecycle and reviewed commit

Existing deterministic Desktop lifecycle tests cover repository generation
changes, stale publication rejection, model-generation changes, reconnect
requirements, connecting/disconnecting behavior, and connection publication
identity. The snapshot command reuses the existing currentness helpers and does
not poll or reconnect. The A-to-B transition is therefore classified non-current
until a fresh B publication matches all host generations; the model equivalent
has the same reconnect requirement.

Existing review lifecycle tests cover identity-not-configured, review-required,
ready-to-authorize, authorized-pending, stale, revoked, unavailable, and
not-applicable presentations. The snapshot exposes state only; authorization
continues through the existing Repository UX and is not consumed or fabricated
by the panel.

The existing verified directory-creation regression proves a successful
repository mutation revokes the commit review and refreshes repository state
while Git status remains clean. The authority command is observational and does
not use Git dirtiness as its refresh/currentness signal.

### Classification, provider metadata, and sanitization

The explicit host classification table covers every current Desktop registry
name: `echo`, `fs.read`, all repository observers, content mutation, file and
directory creation, deletion, rename, and reviewed commit. Unknown names are
not assigned inferred metadata; composition keeps only explicitly classified
entries and the snapshot currentness length check fails closed if registry and
metadata drift.

`echo` remains represented by backend-provided Built-in/Execute classification;
the renderer has no Tool-name security special case. MCP and Process Plugin
presentation is not currently reachable through this Desktop composition path,
so live external-provider UX is NOT APPLICABLE / NOT CURRENTLY REACHABLE. The
closed host-owned DTO contract and existing provider tests keep provider metadata
from granting permission or authority.

Backend serialization tests reject representative paths, executable details,
provider endpoint/credentials, stderr, private aliases, and review internals.
The authority renderer consumes only sanitized DTO fields, uses `textContent`,
and has no raw snapshot/debug rendering or HTML insertion. The authority path
does not consume paths, fingerprints, endpoints, tokens, environment, stderr,
review IDs, selectors, digests, or private aliases.

### Side effects, determinism, and coherency

The snapshot command only copies synchronized host state, immutable composition
metadata, and reviewed-commit presentation. It does not connect, disconnect,
execute Tools, mutate repositories, persist state, increment generations, or
consume authorization. Manual Refresh invokes only
`get_effective_authority_snapshot`; lifecycle refreshes remain attached to their
existing lifecycle effects.

Repeated serialization of one stable snapshot is byte-identical. Tool registry
definitions are sorted by public name. Controlled generation and publication
mismatch tests classify stale/reconnect-required rather than Current. No timing
based race test or provider polling was introduced.

### Refresh isolation and Tauri permissions

Refresh failure replaces the authority view with bounded unavailable/version-
unavailable state and clears prior Current inventory. The failure is handled
locally; it does not disconnect Codex, disable Repository controls, or mutate
authority. Boot sequencing was audited: the refresh helper catches its own
command failure, so the observational feature does not abort unrelated startup.

The generated `allow-get-effective-authority-snapshot` permission is read-only,
has no scope, and is the only new command exposure. The default Desktop
capability contains the expected command with no added filesystem, process, or
authority handles; the deny entry is generated consistently.

## Frontend evidence classification

* EXECUTABLE TESTED: `node --check` for production and test JavaScript; the new
  dependency-free `status_authority_test.js` checks closed status/schema gates,
  Current gating, refresh command isolation, field consumption, text-only DOM
  rendering, and absence of name/generation/HTML inference patterns.
* EXECUTABLE TESTED: Rust snapshot serialization field, determinism,
  sanitization, explicit classification, and existing Desktop lifecycle/review/
  Git-clean mutation suites.
* STATICALLY AUDITED: exact backend/frontend field agreement, lifecycle refresh
  trigger coverage, DOM security, authority-panel secret scope, and packaged
  capability wiring.
* NOT APPLICABLE / NOT CURRENTLY REACHABLE: live MCP/Process Plugin Effective
  Authority UX through the current Desktop registry; no provider functionality
  was added. Windows interactive certification is explicitly deferred to Task
  196.

## Files changed

* `crates/rah-desktop/frontend/status.js`
* `crates/rah-desktop/frontend/status_authority_test.js`
* `crates/rah-desktop/src/effective_authority.rs`
* `docs/plans/2026-09-04-effective-authority-cross-layer-hardening.md`

No Cargo files, lockfile, ADRs, versions, release documents, or `.vscode` files
were changed.

## Validation

Passed:

* `cargo fmt --all -- --check`
* `cargo check --workspace`
* `cargo test --workspace` — all executed tests passed; platform/host-only
  tests remained ignored as configured
* `cargo clippy --workspace --all-targets --all-features -- -D warnings`
* `git diff --check`
* `cargo metadata --no-deps --format-version 1` — 12 packages, all `0.15.0`,
  edition 2024, no dependency drift
* `cargo test -p rah-desktop` — 144 passed, 2 ignored
* `node --check crates/rah-desktop/frontend/status.js`
* `node --check crates/rah-desktop/frontend/status_authority_test.js`
* `node crates/rah-desktop/frontend/status_authority_test.js`
* `cargo build -p rah-desktop --release`

No Windows live UX certification was performed.

## Recommendation

No material deterministic/security blocker remains in the audited path. After
reviewed commit and exact-head CI success, recommend Task 196 — Windows Live
Effective Authority Review UX Validation — as a non-destructive certification
of the observable states described by this contract.
