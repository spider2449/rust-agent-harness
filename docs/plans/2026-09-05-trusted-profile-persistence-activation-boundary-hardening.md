# Task 216: Trusted Profile persistence activation-boundary hardening

## Starting checkpoint

- HEAD and `origin/master`: `87165864308fc2a4fb5e7a686faaf762c3523e30`
- Commit: `feat: add trusted profile restore and forget actions`
- RAH v0.17.0 released; 13 workspace packages; Rust edition 2024
- The worktree contained pre-existing generated-permission line-ending changes;
  those files are outside this task and will be preserved.

## Observed pre-Task216 gap

Connect already captured repository, model, trusted-profile, and connection
generations and performed an early four-generation publication check. However,
the retained `Connected` and pending-publication values did not retain the
captured profile generation, the final publication helper did not recheck the
four-generation tuple, and Effective Authority/chat currentness used only
repository/model (plus partial connection) identity.

## Contract and implementation record

The host-owned currentness identity is the exact tuple:

```text
(repository_generation, model_generation, profile_generation,
 connection_generation)
```

Pending publication must compare all four values immediately before publishing.
Any mismatch rejects and reaps the runtime and provider activation without
changing newer host state. Connected currentness and model-turn dispatch use
the same profile-aware check. Profile generation remains internal to the
currentness proof; no source path or profile bytes are added to IPC.

Connect continues to reread the selected source through
`DesktopProviderActivation::activate(selection)` and derives effective
permissions, external descriptors, provider registry, and Tool composition from
that fresh activation. Restore metadata remains configured/inert presentation
only. A successful connection owns its admitted snapshot; source edits after
Connect do not hot reload or change its generation. Explicit reconnect creates
a fresh activation while leaving profile generation unchanged.

Forget while connected remains preference-only: it removes the remembered path
without changing profile generation, the captured connected tuple, provider
ownership, or effective authority. A remembered-only startup state remains
non-authoritative and Connect does not implicitly Restore it.

## Tests and validation

The implementation will add deterministic coverage for retained profile
generation, matching/mismatched four-generation currentness, stale publication
rejection and provider cleanup, profile-stale chat rejection, fresh source
activation metadata, invalid/missing source failure, no-hot-reload semantics,
Forget-connected behavior, remembered-only inertness, and path privacy.

No authority class, model-visible Tool, persisted-schema field, dependency,
version, or ADR change is planned.

## Completed implementation

- `ConnectionState::Connected` and `PendingConnectedPublication` retain the
  captured profile generation.
- The final publication gate compares repository, model, profile, and
  connection generations while retaining the generation locks through the
  publication decision. Stale pending runtimes and provider activations are
  rejected and shut down; newer host state is not rolled back.
- Effective Authority and repository/turn/replay currentness use the same
  profile-aware tuple. Profile-stale status is `reconnect required`, advertised
  Tools are withdrawn from the current snapshot, and chat dispatch refuses the
  stale connection synchronously and again immediately before runtime start.
- Configured profile counts remain derived from the selected configured
  snapshot; effective external permissions, descriptors, and Tool composition
  remain derived from the fresh Connect-time activation.
- Connect-time fresh source reread and post-Connect no-hot-reload behavior are
  covered through Process Plugin fixtures. Invalid and missing source changes
  fail before provider publication, and provider executables are renameable
  after owner shutdown.
- No IPC schema, persisted schema, path exposure, model visibility, authority,
  dependency, version, or ADR change was made.

## Validation

Passed:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo metadata --no-deps --format-version 1` (13 packages, v0.17.0,
  edition 2024, no dependency drift)
- focused Desktop tuple and source-change tests;
  `cargo test -p rah-profile-composition`, `cargo test -p rah-tools-mcp`, and
  `cargo test -p rah-tools-plugin`
- frontend `node --check` for both status files and
  `node crates/rah-desktop/frontend/status_authority_test.js`

`cargo test --workspace` and `cargo test -p rah-desktop` each reproduced only
the supplied pre-existing Windows
`hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic`
failure; all other Desktop tests passed (171 passed, 2 ignored), including all
Task 216 tests. The non-Desktop workspace run also reproduced the corresponding
pre-existing `rah-tools::git_support::tests::exact_host_safe_directory_is_required_for_foreign_owner_git_status`
failure. No unrelated Git behavior was changed.
