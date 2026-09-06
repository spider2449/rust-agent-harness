# Task 227 — Local Branch Creation Hardening and Release-Gate Matrix

Date: 2026-09-06
Status: IMPLEMENTED — AWAITING EXACT-HEAD CI

## Scope and checkpoint

Task 227 audits and hardens the existing rah-tools implementation of ADR 0020.
It does not change ADR 0020, add authority, or begin Task 228. The only
production implementation file changed is
`crates/rah-tools/src/repository_branch_create.rs`.

The repository was resolved dynamically with `git rev-parse --show-toplevel`.
The origin is `spider2449/rust-agent-harness`, and the starting checkpoint was
`a8f42fcecb039c21e8bbb572fd2d11e0e60a62`, equal to `origin/master`, with a
clean worktree. No fixed checkout path, username, home directory, or temporary
directory is part of the contract.

Host evidence is historical only:

- Platform: Windows
- Git: `2.55.0.windows.5`
- `git init --object-format=sha256`: supported on this host
- Live SHA-256 fixture: exercised by the deterministic rah-tools test

## Hardening changes

- `known_no_effect` now requires fresh target-ref absence, protected HEAD state,
  unchanged pre-existing local heads, and target reflog absence.
- Desired-state-after-uncertain and verified success require one exact,
  host-fixed reflog entry. Git-owned bounded observation rejects missing,
  malformed, wrong, or additional entries.
- A target ref that disappears while its reflog remains is classified as
  `uncertain`; it is never `known_no_effect`.
- Current HEAD and local-head names are observed as bytes. Only ASCII A-Z is
  folded for collision comparison; OIDs remain validated ASCII hex.
- `matches_resources` revalidates the bound repository and Git identities
  without spawning Git, then checks the selected canonical paths.
- The host hooks directory is required to be outside the selected repository,
  and hook identity/emptiness is revalidated immediately before the mutation.
- Explicit `files` ref storage remains the only accepted backend value when
  Git reports one; reftable and other values fail closed.

## Release-gate matrix

| Contract | Production implementation | Deterministic evidence | Status | Notes / limitation |
|---|---|---|---|---|
| Separate authority | Opaque `RepositoryBranchCreationAuthority` owns private policy | Public-surface tests; policy is not public | PASS | No new authority plane beyond ADR 0020 |
| Name-only model input | Closed object parser accepts only `name` | `tool_rejects_closed_input_without_echoing_invalid_name`; schema test | PASS | Repository, OID, ref, argv, hooks, and config are host-owned |
| Execute outer gate | Tool definition uses `PermissionLevel::Execute` | `public_definition_is_closed_and_execute_gated` | PASS | Execute does not construct an authority |
| No switch | Fixed `update-ref` only; HEAD is observed unchanged | Explicit registry creation test | PASS | No checkout/create-and-switch behavior |
| Attached HEAD | Symbolic `HEAD` must be `refs/heads/*`; existing commit OID is captured | `detached_unborn_bare_linked_and_special_state_are_rejected`; successful fixtures | PASS | Current branch bytes are preserved without UTF-8 conversion |
| Dirty/staged allowed | No worktree or index operation | `dirty_staged_and_mixed_states_are_allowed`; repository-plane preservation test | PASS | Changes remain in the current worktree |
| Files ref backend | Git-owned `refs` path must be the ordinary files backend | Normal, explicit files-path fixture; reftable rejection fixture | PASS | Git versions that reject an explicit `extensions.refStorage=files` setting still fail through Git observation |
| Shallow repository | Uses Git-owned HEAD/ref/object observations only | `shallow_sparse_and_alternate_object_repositories_are_admitted` | PASS | Fixture is local and deterministic |
| Sparse checkout | No sparse state is used as a rejection condition | Same sparse fixture | PASS | Sparse checkout is not authority |
| Alternate object storage | Git owns object lookup; no alternate path is trusted as authority | Same `--shared` clone fixture | PASS | Repository identity remains bound to selected root and `.git` |
| Linked/bare rejection | Real `.git` directory, no `commondir`, and non-bare result required | `detached_unborn_bare_linked_and_special_state_are_rejected` | PASS | `.git` file indirection fails closed |
| Selected submodule root | `.git` file indirection is rejected before admission | Same constructor/topology path; no parent scan | PASS | Modern and legacy submodule-root proof is intentionally not broadened; parent repositories are not heuristically scanned |
| Special-state rejection | Git metadata markers for merge, cherry-pick, revert, squash, bisect, rebase, and sequencer states are checked | Merge marker fixture plus production marker list | PASS | These are v1 workflow gates, not a claim Git cannot update refs in those states |
| Name grammar | Closed ASCII 1–128-byte, component, spelling, and forbidden-character validator | `closed_name_contract_and_zero_oid_are_exact` | PASS | Git check-ref-format cannot widen the host language |
| Windows aliases | Case-insensitive `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9` components rejected | Name unit test; Windows extension-like fixture | PASS | No fixed Windows path is asserted |
| Exact/prefix/case collision | Local-head-only Git observation with ASCII byte fold | `exact_prefix_case_and_packed_collisions_are_rejected` | PASS | Tags and remote-tracking refs are outside the collision namespace |
| Packed local heads | `for-each-ref refs/heads/` observes loose and packed heads | Packed collision test | PASS | No raw packed-refs parsing |
| Byte-safe existing refs | Local-head names remain bytes; only ASCII A–Z folds | `local_head_collision_observation_is_byte_safe_and_ascii_only` | PASS | A real non-UTF-8 Git fixture is host-dependent; pure byte tests run everywhere |
| Count/output bounds | `MAX_LOCAL_HEADS=4096`; 512 KiB stdout bound; malformed pairs fail closed | `local_head_count_overflow_fails_before_mutation`; `local_head_output_overflow_fails_before_mutation` | PASS | OID remains UTF-8/ASCII-hex validated |
| Target reflog precondition | Git-owned `reflog exists` must report absence before mutation | `preexisting_target_reflog_fails_admission_without_mutation` | PASS | No raw `.git/logs` access |
| Fixed reflog verification | No max-count shortcut; bounded Git-owned output must parse as exactly one fixed entry | Successful creation; `target_reflog_is_required_for_conservative_outcomes` | PASS | Message, identity, and new OID are host-fixed |
| Known-no-effect proof | Requires target absence and reflog absence plus protected state | Spawn-failure test; precondition and post-effect reflog tests | PASS | Nonzero result alone is never sufficient |
| Desired-state-after-uncertain proof | Requires exact target OID, protected state, unchanged prior heads, and acceptable reflog | Lost-result test produces desired-state status; missing/wrong/ambiguous reflog tests produce `uncertain` | PASS | It remains error-like and never authorizes replay |
| Zero-old SHA-1 | Captured 40-hex OID produces 40 zeroes | `closed_name_contract_and_zero_oid_are_exact`; normal creation | PASS | OID is never model-supplied |
| Zero-old SHA-256 | Captured 64-hex OID produces 64 zeroes | Live `sha256_fixture_uses_a_64_digit_zero_old_cas` | PASS | Git 2.55.0.windows.5 supported the live fixture |
| One mutation | One fixed create-only `update-ref --create-reflog` process after admission | Attempt counters and CAS-race tests | PASS | Test-only fault helpers may construct fixtures; production has one mutating shape |
| No replay | CAS failure, lost result, timeout/observer uncertainty do not retry | `cas_race_is_not_retried_and_lost_result_is_not_verified`; one-attempt assertions | PASS | No fallback primitive |
| No rollback | No delete, restore, branch deletion, reflog deletion, or compensation path | Production source audit and uncertainty tests | PASS | Uncertainty remains conservative |
| Hooks confinement | Unique canonical empty host directory, fixed absolute `core.hooksPath`, identity/emptiness revalidation | Malicious repository hook, local `core.hooksPath`, extra-file, replacement-directory, and cleanup tests | PASS | `--no-verify` is not used |
| Config confinement | System/global config disabled; fixed host config pins hooks, reflog behavior, safe directory, identity, and prompt behavior | Host environment implementation; local hostile hooks-path test | PASS | Repository-local config cannot override host-pinned values |
| Resource identity | Repository and executable identity revalidated before path comparison | Same-resource, different-resource, Git replacement, and repository replacement tests | PASS | `matches_resources` performs no Git process spawn |
| Explicit registry composition | Host composes authority, Tool, then calls `register` | `explicit_authority_tool_registry_composition_creates_exact_branch_without_switching` | PASS | Duplicate registration uses normal registry rejection |
| No implicit authority | `ToolRegistry::new()` does not register branch creation | `registry_has_no_implicit_branch_authority` | PASS | No singleton or startup activation |
| Reusable bounded authority | One authority supports individually admitted requests under the shared lease | Reusable/duplicate-target tests | PASS | Reuse is not unlimited generic ref authority |
| Output privacy | Sanitized closed statuses only; invalid input is not echoed | Tool privacy and closed-disposition tests | PASS | No paths, stderr, argv, environment, hooks, or policy identity |
| Trusted Profile non-authority | No profile capability or provider metadata path constructs this authority | Public export/source audit; no profile files changed | PASS | Profile/provider expansion remains outside this task |
| Desktop/provider expansion | No Desktop, frontend, Tauri, MCP, plugin, runtime, or profile integration | Scope audit; no files in those areas changed | DEFERRED BY ADR | Task 228 owns Desktop / Effective Authority integration |

## Mutation and source audit

The production mutation remains one private, host-built command shape:

```text
git update-ref --create-reflog -m "RAH create local branch" \
  refs/heads/<validated-name> <captured-oid> <zero-oid>
```

The ref, OID, zero OID, reflog message, committer identity, hooks path, cwd,
environment, timeout, and output bounds are host-controlled. Production does
not expose generic `update-ref`, transactions, deletion, force, arbitrary
refs/OIDs, `git branch`, checkout, switch, reset, restore, push, pull, fetch,
or rollback. The test-only Git commands used to construct ambiguous fixtures
are under `cfg(test)` and are not capability paths.

The public surface remains limited to:

- `REPOSITORY_CREATE_BRANCH_TOOL_NAME`;
- `RepositoryBranchCreationAuthority`;
- `RepositoryBranchCreationTool`.

`RepositoryBranchCreationPolicy`, dispositions, reflog helpers, hook helpers,
and zero-OID helpers remain private. No Cargo manifest or lockfile changed.

## Validation evidence

The required validation was run sequentially after implementation. All local
gates passed. On this Windows host, Cargo was run with
`CARGO_BUILD_JOBS=1 RUSTFLAGS=-C debuginfo=0` after an overlapping build had
caused a linker/PDB failure; this changes build diagnostics only and does not
change the test or production configuration.

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test -p rah-tools repository_branch_create -- --nocapture`
- `cargo test -p rah-tools`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`

Observed results:

- `cargo fmt --check`: PASS
- `cargo check --workspace`: PASS
- focused branch-creation tests: PASS, 29 passed
- `cargo test -p rah-tools`: PASS, 177 passed
- `cargo test --workspace`: PASS
- workspace clippy with `-D warnings`: PASS
- `git diff --check`: PASS
- metadata: PASS, 13 packages, all `0.18.0`, edition 2024
- `git diff -- Cargo.toml Cargo.lock`: empty

Exact-head CI is intentionally not claimed in this pre-commit plan. The plan
status remains `IMPLEMENTED — AWAITING EXACT-HEAD CI`; the final Task 227 chat
report records CI only after the pushed Task 227 commit exists.

## Task state

Task 228 — Desktop / Effective Authority Integration — is not started by this
task.
