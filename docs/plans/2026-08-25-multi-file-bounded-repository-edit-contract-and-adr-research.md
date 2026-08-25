# Task 093 — Multi-File Bounded Repository Edit Contract and ADR Research

Status: Complete (research/documentation only)
Date: 2026-08-25

## Starting state

- HEAD and `origin/master`: `a2fa860128618b3a962533d499e29d3ae01fcfa6`.
- Working tree: clean.
- Workspace: 11 packages, all `0.8.0`, Rust edition 2024.
- Certified live platform/baseline: Windows `codex-cli 0.149.0`.
- Ubuntu/Linux: deterministic CI/native-test evidence only.

## Research performed

Inspected v0.8/v0.9 roadmaps, security/architecture, ADRs 0010--0013, Task 083--092 plans, repository patch/create/mutation/stage/unstage/observer/profile code and deterministic tests, native Windows/Unix behavior, CLI composition, Generic Tool Bridge, live fixtures, and release tooling. Implementation confirms that `repo.patch` already uses snapshot multi-replacement semantics within one file, shared lease, raw Git-state preservation, one native attempt, and uncertainty after lost observation; `repo.create-file` is separately exclusive-create with possible retained partial bytes.

## Decision

- Capability: `repo.edit-files`.
- Private policy: `RepositoryMultiFileMutationPolicy`.
- File limit: 1--4; four bounds images, temporaries, attempts, inventory, and partial-state analysis while allowing coherent small source change.
- Request: closed `targets[]`; every target has path, SHA-256, byte length, replacements. No legacy alternate input.
- Order: ascending lexicographic UTF-8 bytes of canonical logical relative path, host-owned.
- Permission: unchanged `PermissionLevel::Execute`; policy is real authority.
- Profile: additive closed profile-v1 binding using existing symbolic Git/repository resources; static is non-effectful and effective composition is fresh-registry atomic.
- Bridge: no production semantic change.

## Failure model

The contract defines `ok`, `invalid_target`, `precondition_failed`, `failed_known_no_effect`, `partial_effect`, and `uncertain`. Partial effect requires full verified committed-prefix and remaining-target inventory; ambiguity, lost observation, changed identity/state, or post-commit cancellation/disconnect without proof is uncertain. No transaction, rollback, automatic retry, replay, or recovery journal. `repo.patch` remains smaller authority; `repo.create-file` remains separate.

## Deliverables

- `docs/RAH_MULTI_FILE_REPOSITORY_EDIT_CONTRACT.md`
- `docs/adr/0014-bounded-multi-file-repository-edit-authority.md` (Proposed)
- This Task 093 plan.

## Follow-up decomposition

1. 094A: pure multi-file preflight/postimage foundations/tests.
2. 094B: native ordered commit, failure semantics, platform fault tests.
3. 094C: audit and ADR acceptance only if evidence matches.
4. 095: profile-v1 and unchanged bridge deterministic integration.
5. 096: baseline/schema hardening.
6. 097: certified Windows live gate.
7. 098--101: v0.9 audit, release preparation, release, cleanup.

## Non-goals

Task 093 makes no Rust, Cargo, dependency, public API, PermissionLevel, profile version, baseline, tag, release, or production implementation change.
