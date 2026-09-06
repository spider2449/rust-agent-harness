# Task 225 - Private Local Branch Creation Policy Foundation

Date: 2026-09-06
Status: COMPLETE

## Repository evidence

- Resolved checkout root: `F:/coding/otherPrj/rust-agent-harness`
- Origin: `https://github.com/spider2449/rust-agent-harness.git`
- Starting `HEAD`, `origin/master`: `30e6be7cea99ba195af8c75a12a7ea0ac4df40eb`
- The repository was clean before Task 225 changes.

## Foundation

Task 225 adds the private, crate-local `RepositoryBranchCreationPolicy` in
`rah-tools`. It is reusable and host-bound to one canonical repository, the
validated native Git executable, the shared repository mutation lease, and a
unique empty host-owned hooks directory. It creates only one previously absent
`refs/heads/<name>` at a freshly captured attached `HEAD` commit through one
fixed expected-absence `git update-ref --create-reflog` invocation.

The policy is not re-exported, does not implement `Tool`, and is not registered
with `ToolRegistry`. `repo.create-branch` remains deferred to Task 226.

## Implemented boundaries

- Closed ASCII branch names: 1-128 bytes, 1-8 components, 1-48 bytes per
  component, ADR 0020 forbidden forms, and Windows base device aliases.
- Git-owned secondary `check-ref-format --branch` admission.
- Normal non-bare repositories with a real `.git` directory, attached existing
  commit `HEAD`, ordinary files ref storage, and loose or packed refs.
- Rejection of detached, unborn, bare, linked-worktree/`.git`-indirection, and
  supported special-operation states.
- Bounded Git-owned local-head observation for exact, ancestor/descendant
  prefix, and uniform ASCII-case local-head collisions.
- Local-head observation is bounded by `MAX_LOCAL_HEADS = 4096` and the fixed
  `HostExecutionPolicy` stdout limit of 512 KiB; overflow fails closed.
- Host-pinned Git environment, safe.directory, hooks path, reflog identity,
  and bounded supervised native process execution.
- Hooks identity and emptiness revalidation with cleanup only for the captured
  unchanged empty directory.
- 40/64-character zero-OID derivation, mandatory CAS, one mutating attempt,
  no retry, no fallback, no rollback, and the closed ADR 0020 outcomes.

## Test evidence

The focused `repository_branch_create` suite ran 12 tests and passed 12/12.
It covers:

- closed name validation and Windows reserved base aliases;
- successful create with HEAD, index, worktree, and existing-ref snapshots;
- Git-owned branch configuration observation proving existing tracking values
  remain unchanged and no `branch.feature/test.remote` or
  `branch.feature/test.merge` values are created;
- clean, unstaged, staged, and mixed repository states;
- exact, ancestor/descendant prefix, ASCII-case, and packed-ref collisions;
- detached, unborn, bare, linked-worktree, and special-state rejection;
- host hook confinement, malicious repository-local `core.hooksPath`
  confinement, and hooks-directory tampering;
- deterministic CAS race, lost-result, unknown-post-state, and spawn-failure
  seams;
- SHA-1 and SHA-256 zero-OID unit logic, including invalid OID rejection;
- reusable policy calls and one-mutating-attempt proof.

The Windows extension-like spellings `CON.txt`, `con.txt`, `NUL.log`, and
`COM1.foo` passed the closed syntax validator but failed safely through Git
admission before branch mutation. ADR 0020 was not broadened. Base aliases
remain rejected by the RAH validator.

Git-owned create/delete did not produce a safe orphan-reflog fixture. Production
still checks reflog admission; no raw `.git/logs` mutation was introduced.

No live SHA-256 repository fixture was exercised on this host; the
format-independent 40/64-character zero-OID logic is deterministically covered.

## Sequential validation

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo test -p rah-tools`: PASS, 160 passed, 0 failed.
- `cargo test --workspace`: PASS, exit code 0; branch-policy tests, `rah-tools`,
  MCP, plugin, runtime, and workspace integration suites passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `git diff --check`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS; 13 packages, all
  version 0.18.0, edition 2024.

`git diff -- Cargo.toml Cargo.lock` is empty. There is no dependency drift.

## Scope and follow-up

Intended changed files are exactly:

- `crates/rah-tools/src/lib.rs`
- `crates/rah-tools/src/repository_branch_create.rs`
- `docs/plans/2026-09-06-private-local-branch-creation-policy-foundation.md`

There are no Desktop, frontend, Tauri, runtime, profile, Cargo, ADR, or
version changes. Task 225A was committed and its exact-head CI passed before
this Task 225 restoration and finalization.

Task 226 - `repo.create-branch` Tool and Host Composition: not started.

## Task 225B post-Task-225 conformance correction

Task 225 was committed and exact-head CI-passed at `ae06ad9` before an
independent ADR 0020 audit identified that the initial implementation observed
all Git namespaces before filtering to local heads. Task 225B corrects only
that observation scope and records the correction separately. The production
snapshot now observes `refs/heads/` only, with an explicit bounded local-head
count in addition to the existing finite Git process-output bound. Branch-name
rules, the fixed expected-absence `update-ref` primitive, CAS, hooks, reflog,
authority boundaries, and the deferred Task 226 Tool exposure remain
unchanged.
