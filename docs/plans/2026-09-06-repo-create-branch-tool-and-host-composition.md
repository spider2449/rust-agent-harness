# Task 226 — repo.create-branch Tool and Host Composition

## Result

Implemented the existing private ADR 0020 branch-creation policy as the
first-party `repo.create-branch` Tool with an opaque host-created authority.
Composition is explicit and bounded; Desktop and Effective Authority remain
unchanged.

## Starting checkpoint

The dynamically resolved repository is `spider2449/rust-agent-harness`.
Starting `HEAD` and `origin/master` are both `fa774161509e4e7d451c5be834b87238b7474df3`,
the Task 225C closure commit. The starting worktree is clean.

## ADR 0020 authority chain

`RepositoryBranchCreationPolicy` remains private and continues to own all
validation, repository observation, shared lease, fixed Git invocation, CAS,
hooks confinement, reflog, and one-attempt outcome semantics. The public
authority owns an `Arc` of that policy and the Tool delegates to it.

## Public Tool surface

Add `REPOSITORY_CREATE_BRANCH_TOOL_NAME`,
`RepositoryBranchCreationAuthority`, and `RepositoryBranchCreationTool` as the
only public branch-creation exports. Do not expose the policy or generic ref
mutation helpers.

## Opaque host authority

`RepositoryBranchCreationAuthority::new(git_executable, repository_root)`
constructs the private policy from host-selected resources. Its fields remain
private. `matches_resources` compares canonical resource identities without
spawning Git or granting authority.

## Tool input contract

The Tool accepts exactly a closed JSON object containing one string property,
`name`. The wrapper performs only shape parsing and passes the exact string to
the policy, which remains authoritative for semantic branch-name validation.

## Tool output contract

Return one JSON `ToolContent` object with `status` and `uncertain`. Verified
success additionally returns validated `name` and full captured `oid`.
Uncertain outcomes remain errors and are never relabeled as verified success.

## Disposition mapping

Map every private disposition to a closed sanitized status: `invalid_input`,
`precondition_failed`, `known_no_effect`, `branch_created_verified`,
`desired_state_observed_after_uncertain_attempt`, or `uncertain`.

## Explicit ToolRegistry composition

Tests construct an authority, pass it to `from_authority`, explicitly register
the Tool, and dispatch it through `ToolRegistry` to create an exact branch.

## Resource binding

Tests use dynamically created repository fixtures to verify matching succeeds
for the bound Git/repository pair and fails for another repository or Git
executable identity.

## No implicit authority

`ToolRegistry::new()` remains empty with respect to `repo.create-branch`; no
global or startup registration is added.

## Reusability

The same authority and Tool create two otherwise admissible branches in one
repository context. Creation does not switch `HEAD`.

## Permission relationship

The Tool definition requires `PermissionLevel::Execute`. Execute remains only
the outer Tool permission gate and does not manufacture branch-creation
authority.

## Privacy / redaction

Rejected arbitrary input, repository paths, Git paths, hooks, argv, environment,
stderr, policy generation, and internal diagnostics are not returned.

## Deterministic tests

Cover the closed definition/schema, explicit registry dispatch, no implicit
registration, resource matching, reusable creation, duplicate rejection,
input privacy, and all closed disposition mappings using existing cfg(test)
policy seams.

## Desktop / Effective Authority deferral

No `rah-desktop` changes are part of Task 226. Effective Authority integration,
repository generation, currentness, and reviewed-commit behavior remain for a
later Desktop task.

## Trusted Profile / provider non-change

No Trusted Profile, MCP/plugin, or provider metadata path can construct this
authority. `rah-runtime-codex` is unchanged.

## Validation

All required gates passed sequentially:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test -p rah-tools repository_branch_create -- --nocapture` (22 passed)
- `cargo test -p rah-tools` (170 unit/integration tests passed)
- `cargo test --workspace` (workspace suites passed; host-only tests ignored)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`

Metadata reports 13 packages, all version `0.18.0`, all edition 2024. The
`Cargo.toml` and `Cargo.lock` diff is empty.

## Commit

The actual Task 226 files are committed as:
`feat: expose repo create-branch tool`.

## Exact-head CI

Push `master`, then verify CI success for the exact Task 226 commit SHA.

## Next task

Task 227 — Local Branch Creation Hardening and Release-Gate Matrix. Do not start
it automatically.
