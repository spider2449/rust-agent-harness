# Task 071: Multi-Replacement `repo.patch` Contract and ADR 0012 Research

Status: Complete (research/design only)
Date: 2026-08-24

## Scope

Task 071 defines the v0.7 contract for bounded multiple exact literal
replacements in one existing clean HEAD-tracked UTF-8 worktree file. It makes
no Rust, Cargo, dependency, public API, permission, profile, bridge,
repository-create/delete/rename, or live-validation change.

## Decision

The approved contract is in
`docs/RAH_V0.7_MULTI_REPLACEMENT_PATCH_RESEARCH.md`:

- Extend `repo.patch` in place, retain the legacy single-replacement request,
  and add a mutually exclusive bounded `replacements` array.
- Match every old text exactly once in the original snapshot, reject duplicate
  entries and overlapping ranges, then build one postimage ordered by original
  byte offset.
- Cap the array at 16; retain the 64 KiB serialized/text and 1 MiB
  input/final-file limits; add a 64 KiB aggregate replacement-text budget.
- Retain the repository lease, host-owned same-directory temporary file,
  immediate pre-commit revalidation, one native-replacement commit point, and
  exact post-write verification.
- Amend ADR 0012 narrowly before implementation. Keep
  `PermissionLevel::Execute`, the trusted-profile schema/version, and Generic
  Tool Bridge production code unchanged.

Task 072 is only deterministic implementation plus Windows/Ubuntu coverage for
this contract. It does not begin profile, bridge, or live feature work.

## Validation gate

Run the required workspace commands, `git diff --check`, and final Git
status/diff inspection. Commit the documentation-only change, push it, and
verify the Ubuntu CI workflow is green.
