# Task 058 plan: repository observer research

Date: 2026-08-23
Status: Complete

## Scope

Documentation-only research for `repo.status`, `repo.diff`, `repo.diff-staged`, and `repo.file-info`. No Rust/Cargo, trusted-profile, bridge, live-example, mutation-authority, or ADR change is in scope.

## Completed work

1. Inspected the v0.6 roadmap, accepted repository/trusted-profile ADRs, host-execution/Git support, repository identity/leases, schemas/output limits, and Windows process controls.
2. Researched Git porcelain/plumbing, helper suppression, optional locks, environment/configuration, paths, sparse flags, binary data, and unborn HEAD.
3. Defined exact command families, closed schemas, fail-closed bounds, staleness/read-only language, security handling, and Windows/Unix tests in `docs/RAH_V0.6_REPOSITORY_OBSERVER_RESEARCH.md`.
4. Decided no ADR and retained `PermissionLevel::Execute`; recommended a `repo.file-info`-first Task 059 implementation sequence.

## Validation and stop

Run the required documentation/Cargo metadata checks, inspect diff/status, then commit only these two documents. Stop after Task 058; no implementation begins.
