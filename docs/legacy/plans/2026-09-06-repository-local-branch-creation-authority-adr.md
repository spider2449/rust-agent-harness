# Task 224 — ADR 0020 Repository Local Branch Creation Authority

## Result

Record Task 223's settled v0.19 contract as accepted ADR 0020.  This task is
documentation only and starts no implementation.

## Starting checkpoint

Start from clean `e1884d46d8433aa4f3e5c3f65c8d86a88eb5350e`, equal to
`origin/master`, following released v0.18.0.  ADRs ended at 0019; 0020 was
unused.

## Why a new ADR is required

Creating a local ref is a new persistent authority plane, distinct from the
existing index, worktree, file/directory, and reviewed-commit planes.

## Authority boundary

Accept private reusable `RepositoryBranchCreationPolicy` for exactly one
previously absent `refs/heads/<validated-name>` at captured attached HEAD.
It is not generic Git/ref, commit, checkout, index, worktree, remote, or
repository-selection authority.

## Model / host / frontend ownership

The model supplies one closed logical name.  The host owns repository, Git,
identity, cwd, environment, ref namespace, OIDs, CAS, hooks/config, and argv.
Frontend, Tool registration, Execute, Trusted Profile, and provider metadata
cannot create the policy.

## Persistent effect

Only the new direct local head and its fixed Git-owned reflog are allowed.
HEAD/current branch, index, worktree, tracking, remotes, conversation, and
provider state remain unchanged.  Creation never switches a branch.

## Git primitive and CAS

Specify private fixed `git update-ref`, a host-built `refs/heads/` name,
captured OID, and zero-old-OID expected-absence CAS (40 SHA-1 or 64 SHA-256
zeroes).  No generic update-ref, force, delete/recreate, fallback, or retry.

## Hook/config confinement

Require reviewed-commit-equivalent host-owned empty hooks confinement and
host-pinned security-critical Git configuration.  `--no-verify` is not a
ref-hook control.  Fixed reflog message/identity and `--create-reflog` bound
the permitted secondary effect.

## Repository/name admission

Admit normal attached non-bare `.git` repositories and dirty/staged state;
reject detached/unborn, bare, indirection/linked worktree, selected submodule,
unsupported topology/storage including reftable, and hardened special states.
Use Task 223's closed ASCII 1–128 byte, component, reserved-name, no-`refs/`,
no-normalization, and bounded case/prefix collision contract.

## Uncertain-effect semantics

One mutating attempt only, with fresh revalidation under the repository lease,
CAS, and post-observation.  No replay, rollback, deletion, or compensation.
Record the six-state sanitized taxonomy, including that desired state observed
after an uncertain attempt is not verified RAH success.

## Review/currentness decision

Successful creation preserves a valid reviewed-commit authorization, does not
increment repository generation, does not stale a current connection, and does
not alter conversation or provider composition.

## Relationship to ADRs 0010–0019

ADR 0020 adds local branch creation to the established separate-authority
taxonomy, retaining ADR 0010 index, ADR 0011 composition, ADR 0016 commit,
and ADRs 0012–0019 worktree-entry boundaries.

## Documentation changes

- Add `docs/adr/0020-repository-local-branch-creation-authority.md`.
- Add this Task 224 plan.
- Do not change release-facing architecture/security documents: they describe
  v0.18.0 and contain no contradictory authority-plane list.

## Validation

Run `cargo fmt --check`, `cargo check --workspace`, `git diff --check`, and
`cargo metadata --no-deps --format-version 1`.  Confirm 13 packages, version
0.18.0, edition 2024, and no dependency drift.  No live validation applies.

## Commit

Commit documentation only as `docs: define local branch creation authority`.

## Exact-head CI

Push master, require clean `HEAD == origin/master`, and await completed/success
CI for that exact commit.

## Next task

Task 225 — Private Local Branch Creation Policy Foundation.  It may implement
the private `rah-tools` foundation with deterministic tests, but is not started
by Task 224.
