# Task 214 — Persist inert Trusted Profile preference

## Contract

Task 213 selects Option B: Desktop startup restores only a remembered Trusted
Profile source path. Startup does not read, stat, canonicalize, validate, or
load the profile source; it does not create a selected profile, increment the
Trusted Profile generation, spawn or compose providers, publish external
Effective Authority, or advertise external Tools. Explicit Restore is a later
host action and explicit Connect remains the provider activation boundary.

## Implementation

`desktop-preferences.json` advances to closed schema version 3:

```json
{
  "version": 3,
  "model": { "provider": "inherit" },
  "commit_identity": { "name": "...", "email": "..." },
  "trusted_profile": { "path": "C:\\profiles\\provider.json" }
}
```

`commit_identity` and `trusted_profile` are optional and omitted rather than
written as `null`; root key order is version, model, commit_identity, then
trusted_profile. Version 1 remains model-only, version 2 retains its optional
commit identity, and both reject a Trusted Profile field. Version 3 requires a
valid model and accepts the optional identity and remembered path. Unknown
versions, fields, duplicates, nulls, wrong types, malformed content, invalid
UTF-8/BOM, empty files, and oversized files fail closed.

The private `RememberedTrustedProfilePath` semantic type represents only a
syntactically valid inert persisted native path. It requires a non-empty,
absolute, UTF-8 path of at most 1024 UTF-8 bytes, rejects CurDir and ParentDir
components, and performs lexical checks only. Windows accepts ordinary
drive-letter absolute paths and rejects UNC, verbatim/device/non-disk
prefixes and ADS-style colons in normal components. The original string,
separator spelling, and drive-letter case are preserved. No canonicalization
or source topology check is performed.

`Preferences` retains the remembered path alongside commit identity. Model
save, identity save, path save, path forget, and model reset each rewrite the
complete preference document while preserving unrelated planes. In-memory
state is changed only after the existing atomic writer succeeds. No
interprocess locking is added; existing last-writer-wins/unsupported
concurrent-writer semantics remain unchanged.

## Tests

Deterministic tests cover v1/v2 compatibility, v3 presence/absence,
canonical version/order/omission, strict rejection cases, lexical path rules,
Unicode and size bounds, preservation and v1/v2 upgrade, profile save/forget
atomic failure invariants, and startup loading of a missing inert path without
profile-source I/O or Desktop profile/provider activation effects. Existing
writer seams cover temp creation, write, sync, replacement, and the Windows
access-denied fallback.

## Scope and authority

Only the private Desktop preference implementation and a focused Desktop
startup test are changed. The preference file is non-authoritative configuration
and the remembered path is not exposed
through Effective Authority, dynamic Tools, prompts, Tool schemas, provider
metadata, conversation storage, or frontend IPC. No PermissionLevel,
repository/provider/frontend authority, dependency, ADR, release-facing
historical documentation, provider activation behavior, or Connect behavior is
added or changed.

## Validation

- `cargo fmt --check` — passed.
- `cargo check --workspace` — passed; the three future-facing private APIs
  have scoped dead-code allowances because Task 215 will consume them.
- `cargo test -p rah-desktop --bin rah-desktop desktop_preferences` — passed,
  23 tests.
- `cargo test -p rah-desktop --bin rah-desktop
  startup_remembers_profile_path_without_selecting_or_activating_it` — passed.
- `cargo test --workspace` — all Task 214 tests and all other workspace tests
  passed; the known unrelated Windows foreign-owner Git diagnostic
  `tests::hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic`
  failed as documented by the task instructions; two host-only tests were
  ignored.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed.
- `git diff --check` — passed.
- `cargo metadata --no-deps --format-version 1` — passed; 13 packages remain
  at version 0.17.0 and edition 2024 with no dependency drift.
