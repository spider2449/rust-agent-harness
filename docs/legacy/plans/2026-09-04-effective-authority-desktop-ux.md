# Task 194 — Effective Authority Desktop UX

Date: 2026-09-04  
Status: implementation complete  
Starting HEAD: `847c9c12ac04ad8e01dfdfb08813a13093db379b`

## Scope and baseline

Task 194 adds a read-only Desktop presentation for the sanitized Task 193
`get_effective_authority_snapshot` command. Task 192 remains the UX and
security contract. Task 193 is the backend baseline; its files were not
changed. The released v0.15.0 commit and tag remain untouched.

No authority, lifecycle, repository, profile, provider, commit-review, or
persistence behavior was added. No command, dependency, version, ADR, or
release metadata was changed.

## UX

The Effective Authority section is placed after Repository in the main status
card, near Runtime and Repository context. It contains one manual `Refresh
Authority` button whose only invocation is
`get_effective_authority_snapshot`. It has no grant, revoke, enable, disable,
connect, reload, execute, repository-switch, or commit-authorization control.

The primary summary renders the backend status, safe repository display name,
binding, runtime kind/source, effective-tool count, unavailable count, and
reviewed-commit state. Status wording covers all seven closed states:

* `no_repository`: No repository selected
* `disconnected`: Runtime disconnected
* `connecting`: Connecting — effective runtime inventory pending
* `connected_current`: Current
* `reconnect_required`: Reconnect required
* `stale`: Stale — not current authority
* `unavailable`: Authority snapshot unavailable

Unknown statuses and schema versions fail closed. The word Current is emitted
only for `connected_current`; currentness is never derived from generations or
connection UI state. Stale and reconnect-required snapshots receive an
additional not-current explanation.

Configured, Effective, and Advertised are explained as separate concepts.
Configured counts are shown only under Advanced context and are never used as
the effective count. Tool rows use the exact public name and backend fields for
source kind/label, effect class, authority category, dispatch permission,
repository binding, and advertised state. Closed frontend mappings provide
bounded human labels. `echo` is retained and shown using its backend
backend-provided Built-in/Execute classification. The DTO does not serialize a
separate development/test label, so no frontend Tool-name special case was
added and no security-relevant distinction is inferred.

Unavailable capabilities are rendered in a separate list with bounded state
and reason labels. Reviewed commit is rendered as state only, including
`authorized_pending`; the existing Repository staged-review authorization
control remains separate and unchanged.

The panel uses semantic sections/headings, description lists, real lists,
explicit button types, and a polite live status line. Long names wrap, lists
grow without fixed-height clipping, and styles collapse naturally at narrow
widths. Permanent copy explains that the inventory is informational and
requests remain subject to host permission and policy checks.

## Sanitization and refresh behavior

The renderer consumes only the snapshot DTO. It does not read repository path,
fingerprint, endpoint, executable, environment, stderr, private alias,
provider description, or review authorization fields. It does not infer
authority from Tool names, permissions, advertised state, or generations.

Authority refreshes occur after initial boot, connect/disconnect completion,
repository selection, model configuration apply/reset, commit authorization,
repository stage/unstage refreshes, repository refresh, and the existing
`repository_snapshot_refresh` event. These refreshes remain independent from
repository rendering. No polling or retry loop was added.

If the command fails, the prior snapshot is cleared and replaced with the
bounded “Effective authority snapshot unavailable” presentation, so an old
Current state cannot remain visible. Unknown enum labels also use conservative
“Unknown / unavailable” text.

## Validation

The repository has no frontend unit-test harness or DOM test dependency. The
frontend validation is therefore code-review/static validation plus the
repository-established Node syntax check; no executable frontend behavior
claims are made beyond those checks. The pure renderer helpers keep mapping
and refresh behavior deterministic and avoid invocation from row renderers.

Commands run for this task:

* `node --check crates/rah-desktop/frontend/status.js`
* `cargo fmt --check`
* `cargo check --workspace`
* `cargo test --workspace`
* `cargo clippy --workspace --all-targets --all-features -- -D warnings`
* `git diff --check`
* `cargo metadata --no-deps --format-version 1`
* `cargo test -p rah-desktop`
* `cargo build -p rah-desktop --release`

Results: all commands passed. `cargo test --workspace` passed across the
workspace; the Desktop portion was 142 passed and 2 ignored. The focused
`cargo test -p rah-desktop` result was also 142 passed and 2 ignored. The
release Desktop build completed successfully. `cargo metadata` reported 12
packages, version 0.15.0, edition 2024, and no dependency drift. No Windows
live Desktop validation was performed; that belongs to the separately scoped
Task 196.

## Files and follow-up

Changed files are the static frontend `index.html`, `status.js`, `styles.css`,
and this plan. Task 193 backend files, Cargo manifests/lockfile, ADRs,
versions, release state, and `.vscode` were not changed. Workspace metadata
remains 12 packages, version 0.15.0, edition 2024, with no new dependencies.

Recommended next task: Task 195 — Effective Authority Cross-Layer
Deterministic and Security Hardening. Do not begin it automatically.
