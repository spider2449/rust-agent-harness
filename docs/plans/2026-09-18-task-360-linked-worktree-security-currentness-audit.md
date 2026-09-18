# Task 360 — Linked Worktree Security and Currentness Audit

Status: audit and deterministic hardening; Task 361 Windows live certification remains separate

Checkpoint audited: `5654bff96baacfd0d5e8933f146ade6e93ac85eb`

Task 359 production source: `02c3eed1c865e38a635ee8ba0f3f6caee1d27a16`

Task 359 test descendants: `3d7e7bdaf2224ee8b6095c110e0d648e9b5da9a2`, `5654bff96baacfd0d5e8933f146ade6e93ac85eb`

Accepted authority: ADR 0027 and ADR 0029. Both ADRs remain unchanged.

## Scope and independent method

The audit followed the implementation from `RepositoryAdmissionIdentity` and
`RepositoryGitLayout` through Desktop admission/activation and the existing
observer, content, index, branch, and reviewed Commit policies. Task 359 plan
claims were treated as search leads, not proof. The audit inspected parser and
process boundaries, exercised real Git main/A/B fixtures, tested malformed
porcelain records directly, checked linked currentness paths, and reviewed
test-only admission and Stage hooks.

The production boundary remains one human-selected root and one active
composition. No `PermissionLevel`, mutation class, Tool input, selector,
provider route, persistence field, or frontend control was added.

## Trust chain and retained evidence

| Stage | Stable identity retained or checked | Semantic Git evidence | Mutable currentness and publication guarded |
| --- | --- | --- | --- |
| Human-selected root capture | Canonical root and filesystem object identity; exact canonical Git executable, object identity, size, and modified time | None at capture | Filesystem aliases and reparse ancestry rejected before an identity is returned |
| Layout classification | Main `.git` directory identity, or linked `.git` file identity/bytes, private/common directory identities, `worktrees` and registration identities, backlink and `commondir` identities/bytes | Closed main or linked shape; linked target/backlink resolve to the selected root and exact standard common relationship | Any changed/replaced retained object or content makes the retained layout stale |
| Fixed semantic validation | Rechecks filesystem layout before and after probes | Selected top-level root, private/common dirs, non-bare and non-submodule status, selected index/HEAD Git paths, and exactly one matching porcelain registration | Rejects semantic disagreement before Desktop constructs/publishes an admitted member or active composition |
| Repository construction | Receives the host-canonical selected root and retained executable | Individual capability constructors bind that root/layout | Construction failure happens before membership publication |
| Final admission | Recaptures the complete identity and repeats semantic probes | Same closed Git relationship after construction | Only then does the synchronous duplicate/nesting check publish inert membership |
| Activation | Retained member identity plus member/admission/repository generations | Full semantic validation before construction, after construction, and after the deterministic prepublication barrier | Final membership/currentness check publishes the new active-only composition; failure leaves the prior active composition and generation intact |
| Capability effect | Same selected root/private Git relation is revalidated from captured filesystem identities and exact relationship-file contents | Each capability separately revalidates its selected HEAD/index/ref/target facts; Commit and branch mutation also use full semantic layout validation at their final Git gate | Does not lock external Git. Native Git and filesystem races remain subject to existing stale/uncertain handling; no race-free external-Git claim is made |

Main repositories classify from a real `.git` directory. Linked-only files
(`commondir`, backlink, worktree registration) are never required for the main
form. Ordinary main-root observation and mutation remain rooted in the same
canonical root and preserve existing policies.

## Gitfile and registration parser

Gitfile, `commondir`, and backlink content is read with a 4 KiB cap, a
`limit + 1` bounded read, strict UTF-8, NUL rejection, one-record parsing, and
CR/LF rejection other than one terminal LF. Evidence retains both exact bytes
and filesystem identity. `commondir` remains restricted to the standard
`../..` relation. Root, private, common, worktrees, registration, and relevant
ancestors reject symlinks/reparse points and replacements.

The porcelain `-z` parser is bounded by the 1 MiB stdout policy and rejects
truncated records, duplicate fields, missing `worktree`/`HEAD`, malformed OIDs,
conflicting or absent branch/detached state, unknown fields, non-UTF-8 paths,
and a duplicate canonical selected root. A selected prunable or bare record
fails. Locked records, with or without a reason, remain valid when present and
coherent. Any malformed record fails closed rather than selecting the first
apparently matching record.

New table-driven parser cases cover missing fields, duplicate fields, both and
neither branch/detached, duplicate matching roots, malformed OID, unknown
field, locked reason, prunable reason, non-UTF-8 path, empty list, and truncation.
Allocation is bounded by the checked 1 MiB child-output limit; no parser panic
or unbounded input path was found.

## Absolute and relative gitfile spelling

Task 359 rejected relative `gitdir:` and backlink records even if they resolved
to the retained target. That was narrower than ADR 0029, which requires the
target to resolve to the accepted canonical private gitdir and the backlink to
resolve to the selected `.git`, without requiring absolute spelling. Git's
[worktree documentation](https://github.com/git/git/blob/master/Documentation/git-worktree.adoc)
supports `--relative-paths` and the `worktree.useRelativePaths` setting;
therefore native Git does not always
produce absolute linking files under every supported configuration. The local
native Git `worktree add` default emitted an absolute `.git` record, while a
real fixture with both relative linking records was accepted by Git and passed
RAH's fixed semantic probes.

Correction: relative spellings are now resolved against the containing
`.git`/`gitdir` directory, then subjected to the same canonical target,
standard registration, ancestry, private/common, and semantic checks. Windows
rooted or drive-prefixed non-absolute spellings remain rejected. This accepts
only the complete ADR 0029 linked relationship; it does not accept arbitrary
gitfiles.

## Fixed Git probe boundary

All linked classification probes use the retained host-selected executable,
exact fixed arguments, selected-root cwd, the controlled observer environment,
no shell, a five-second timeout per process, bounded stdout/stderr and combined
output, and owned process execution. Limits remain: gitfile, commondir, and
backlink 4 KiB each; rev-parse stdout 16 KiB; probe stderr 16 KiB; worktree
list stdout 1 MiB. Overflow, timeout, nonzero exit, invalid UTF-8 where paths
are parsed, and semantic disagreement fail closed.

The Task 359 source diff contains no new production `Command::new` path.
Production semantic probes route through `HostExecutionPolicy`; direct
`Command` calls found in the linked fixture/parser and Stage seam are
`#[cfg(test)]`. Production invokes no worktree add/remove/prune/repair/move/
lock/unlock command and no fetch/pull/push/clone behavior.

## Effect-time relationship and currentness

The filesystem revalidation is sufficient for the stable linked registration
relation because that relation is the exact retained root gitfile, private
directory, common directory, `worktrees` parent, registration directory,
commondir, backlink, and their identities/contents. Native move, remove,
prune, repair, replacement, or relation rewrites necessarily change one of
those captured facts. A lock marker is deliberately not identity: present,
coherent locked registrations remain valid, and lock state grants no authority.
No global common-directory generation or external-Git lock is introduced.

| Capability | Final identity check | Capability-specific currentness |
| --- | --- | --- |
| Patch | Stable layout revalidation at the last filesystem replacement boundary | Full Git layout and selected HEAD/index/target snapshot are validated during retained preparation revalidation; selected target identity/content is rechecked before replacement |
| Multi-file edit | Full semantic layout validation before each target effect and after each committed target | Selected root, HEAD/index snapshot, path identities, and ordered postimages; unrelated shared refs are not global invalidators |
| Create file / create directory | Stable layout revalidation at filesystem creation boundary; full layout validation in immediately preceding state capture | Selected root, parent chain, absence, selected HEAD/index, and postcondition; no sibling Git path is targeted |
| Delete / rename | Stable layout revalidation at native filesystem boundary; full semantic and operation snapshot validation in the final reviewed capture | Selected target, index, HEAD/branch as required by the existing reviewed policy, parent identities, and no-replace/absence postconditions |
| Stage / Unstage | Stable layout revalidation immediately before native Git; full semantic validation during pre/post state capture | Selected HEAD, index, target, and worktree snapshot. Unrelated B branch commits do not stale A; unrelated A index mutations retain the existing policy-violation/uncertain result |
| Commit | Full semantic layout validation at reviewed snapshot and before Commit spawn | Selected private index, selected HEAD, attached branch and expected branch OID, staged entries/tree, Commit identity/generation, hooks, and executable identity |
| Branch creation | Full semantic validation before fixed Git ref mutation | Current selected HEAD/source, exact requested target-ref precondition and CAS result; unrelated refs do not invalidate unless they collide with source/target rules |
| Observation / diff | Full semantic validation before observations; stable identity rechecked around publication | Only selected-root HEAD/index/worktree observations; no sibling/private path is exposed |

Filesystem-only final layout checks are not treated as operation-state checks.
Each effect retains its prior HEAD/index/ref/target validation. Git itself
serializes worktree-local index writes and supplies ref compare-and-swap for
shared branch mutations. The process-local RAH lease is keyed by canonical
selected root (ASCII-folded on Windows), so it serializes RAH operations within
one worktree. It intentionally does not merge A/B leases because their indexes
are private, and it does not claim to serialize shared refs against other
worktrees or external Git clients. Shared-ref safety comes from capability
currentness/CAS, not from a common-directory lease.

## Stage, Unstage, Commit, and branch findings

Stage's removed broad `show-ref` snapshot is correct: an unrelated B commit
does not stale A, while A's selected HEAD/index/target and full selected-root
registration evidence remain checked. The linked A Stage test first advances
B's branch, then stages A and verifies A's index changes while main and B
remain byte-equivalent. It also leaves B content staged while Unstage changes
only A. No `index.lock` remains for main, A, or B after either operation.

The linked Commit fixture captures A's review, commits on B, adds separate
staged B content, and commits A. It verifies A's branch advances, B's HEAD and
branch remain unchanged, B staged state and index remain unchanged, and the
main index remains unchanged. A separate test moves A's selected branch ref;
the old review fails and the native Commit attempt counter remains zero.
Detached linked HEAD remains usable under existing non-Commit policies;
Commit still requires an attached branch.

Branch creation uses the selected A HEAD as source, writes one shared local
branch through the fixed ref CAS path, does not switch A or B, and does not
invoke worktree lifecycle commands. Existing target-ref race/no-retry tests
remain the evidence for conflict classification.

## Membership, submodules, separate directories, and lifecycle

`RepositoryAdmissionIdentity::relation` treats the same root filesystem object
or same validated private target as duplicate, parent/child roots as nested,
and different private targets as distinct even when common-dir matches. It
does not use a registration basename as authority. Copied gitfiles fail their
backlink relation. Same A root/alias is duplicate; main/A/B share common Git
storage but remain three members. Modern and old-form submodules fail through
superproject and closed private/common/backlink/registration checks. A real
`--separate-git-dir` fixture lacks the full accepted linked relation and is
rejected. No single file-shape heuristic authorizes a candidate.

Captured root, `.git`, private/common directory, registration, backlink,
commondir, and Git executable replacement/currentness tests fail stale. Real
external `git worktree move` and remove fixtures do not migrate the retained
identity. Locked and present is accepted; missing or prunable is rejected.
Windows junction coverage rejects root, root `.git`, private, common, and
registration ancestry cases. Root aliases are reduced by canonical path and
filesystem identity. This is deterministic reparse coverage on Windows, not a
race-free TOCTOU claim.

The active root's own `.git` file is skipped as root metadata by nested-boundary
checks. Nested `.git` directories, nested gitfiles, submodules, and linked-root
content traversal remain blocked. Representative read, patch, create, delete,
rename, multi-edit, and Stage snapshot tests stay within the selected root.

Admission performs all fallible capture/probe/construction/final validation
before `admit_repository_identity`; membership publication is one locked
operation after duplicate/nesting checks. Failure cannot publish a member,
active composition, ToolRegistry, provider, or runtime. Activation builds a
fresh composition off-publication, then rechecks the retained member,
generations, and identity before its synchronous commit point. The previous
active composition remains current when target activation fails. Close and
inactive removal leave the worktree root, registration, index, and refs alone.

## Production/test parity and test-hook ownership

Production Windows Desktop admission runs async semantic validation before
construction and full semantic revalidation afterward. The old `#[cfg(test)]`
synchronous helper could admit a linked gitfile without running those probes.
The linked Desktop lifecycle test now calls the same factored async admission
sequence as production. The old helper explicitly refuses a `.git` regular
file, so a test cannot claim Desktop linked admission through that shortcut.
The real linked Desktop test checks failed shortcut admission publishes no
member, then admits main/A/B through the production validation sequence.
Direct policy tests that intentionally construct policies from fixed roots
test capability behavior only, not Desktop admission.

Stage/Unstage failure hooks are now stored by canonical root in a test-only
map. A concurrent A/B test installs distinct hooks, runs B first, and proves B
cannot consume A's hook; the test then observes each hook's intended behavior.
There is no test-hook semaphore or permit lifetime that can block another
repository's test. Production Stage/Unstage code has no hook path.

`rah-runtime-codex/src/bridge_tests.rs` remains test-only adaptation. No
production Codex routing or model-controlled repository selection changed.

## Isolation, authority, privacy, and persistence

Observers bind their Git cwd to the selected root and use that worktree's
private HEAD/index. The linked observer test uses distinct A/B state. Effective
Authority stays active-member-only; switching creates fresh composition,
Close withdraws repository authority, and restart begins with zero members and
no active repository authority. Membership/authority DTO tests exclude private
Git paths and identity objects. Tool schemas contain no member/worktree
selector. HostExplicit remains exactly the existing eleven eligible names;
`repo.create-directory` remains ineligible.

Layout errors are generic and bounded. Private/common paths, gitfile bytes,
registration IDs, filesystem IDs, and raw Git stderr do not enter product DTOs
or Tool schemas. Existing selected-root display presentation remains the only
approved path presentation. Linked metadata is process-local and is not added
to remembered-workspace persistence. No schema, Cargo dependency, or lockfile
change was made.

## Deterministic race matrix

| Race | Deterministic evidence and result |
| --- | --- |
| Admission vs gitfile replacement | Same-object in-place gitfile mutation and replacement make retained identity stale; failed classification is before membership publication |
| Admission vs registration change | Native move/remove make retained identity stale; Desktop linked admission validates before inert publication |
| Activation vs gitfile rewrite | `task360_linked_activation_rejects_gitfile_change_at_publication_barrier` rewrites B's root `.git` to A's registration at the deterministic final barrier; B fails stale and A's active composition/generation remain unchanged |
| Activation vs private-dir replacement | Private directory replacement stales retained identity; final activation gate repeats semantic validation |
| Activation vs external move/removal | Real external move/remove stale retained identity; target activation rejection preserves the old active member/generation |
| Stage A vs unrelated B branch Commit | Linked Stage fixture commits B before Stage A; A Stage succeeds and A alone changes index |
| Stage A vs A HEAD change | Stage captures selected HEAD at execution and rejects stale selected state; related selected-HEAD retained-preparation tests cover patch/multi-edit currentness |
| Unstage A vs staged B state | B has a staged entry while A is staged then unstaged; main/B indexes remain byte-equivalent |
| Commit review A vs B Commit | Linked Commit review is captured before B commits; A review remains valid and A Commit leaves B HEAD/index/staged state intact |
| Commit review A vs A branch-ref movement | Selected A ref movement rejects old review; Commit spawn attempts remain zero |
| Branch creation A vs target-ref race | Existing deterministic CAS-race test refuses retry/rebinding |
| Authoring preparation A vs stale linked metadata | Retained linked-layout byte/object revalidation rejects stale identity; effect policies revalidate it before file mutation |
| Switch A→B vs external A removal | Switching only publishes after B semantic validation and lifecycle currentness; external A removal does not retarget B or grant sibling authority |
| Close A vs external Git observation | Close is membership/composition withdrawal only; linked lifecycle test compares registration bytes before/after Close |
| Stage hook A vs concurrent B | New two-root hook test proves distinct test hooks remain bound to canonical A/B roots |

No sleep-based race proof was introduced. The reviewed code makes no claim
that external Git cannot race after a final check.

## Defects and corrections

1. Relative gitfile/backlink spelling was rejected despite being a valid native
   Git linked-worktree form under documented configuration. Corrected by
   resolving relative spellings and keeping the full closed relation checks.
2. Desktop's test-only synchronous admission helper bypassed semantic probes.
   Corrected by routing the linked Desktop lifecycle test through the same
   factored async validation sequence as production and making the shortcut
   reject linked `.git` files.
3. The global Stage test-hook semaphore serialized different repositories and
   could remain held behind an unmatched hook. Corrected test-only storage to
   a canonical-root map and added a deterministic A/B ownership test.
4. The linked Desktop suite lacked a metadata rewrite exactly after composition
   construction. Added a deterministic final-publication barrier case proving
   that linked gitfile replacement cannot publish the stale composition.
5. Desktop Clippy found a pre-existing late-initialized variable in a test-only
   live-gate match. Replaced it with the match expression result; no production
   behavior changed.

No contradiction with ADR 0029 remains. No new authority or dependency was
introduced.

## Validation and baseline drift

Initial focused validation completed:

- `cargo fmt --check` — PASS.
- `cargo test -p rah-tools repository_git_layout::tests -- --test-threads=1` —
  PASS after correcting a malformed parser-test fixture.
- `cargo test -p rah-tools stage_test_hooks_are_owned_by_their_canonical_repository_root -- --test-threads=1` — PASS.
- `cargo test -p rah-desktop task_359_linked_worktrees_use_one_revalidated_active_composition -- --test-threads=1` — PASS.
- `cargo check -p rah-tools` — PASS.
- `cargo check -p rah-desktop` — PASS.
- `cargo clippy -p rah-tools --all-targets --all-features -- -D warnings` — PASS.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` —
  PASS after the test-only lint correction.
- `cargo test -p rah-tools -- --test-threads=1` — PASS, 319 unit tests plus
  integration and documentation tests; two environment-gated tests ignored.

The final focused linked activation barrier test also passes after the
test-only Clippy correction:

- `cargo test -p rah-desktop task360_linked_activation_rejects_gitfile_change_at_publication_barrier -- --test-threads=1` — PASS.

Full local workspace gates:

- `cargo fmt --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace -- --test-threads=1` — PASS. Desktop: 313 passed,
  15 expected host/live tests ignored. `rah-tools`: 319 unit tests passed;
  integration and documentation tests passed. Codex bridge: 83 passed, one
  host-only test ignored. All other workspace unit, integration, and doc-test
  commands completed successfully; only explicit executable/live environment
  gates were ignored.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  PASS after correcting one needless borrow in the new test assertion.
- `git diff --check` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 packages, every
  package `0.29.0`, edition 2024.
- `cargo check -p rah-tools`, `cargo check -p rah-desktop`, package Clippy,
  parser/Stage focused tests, and the complete `rah-tools` suite — PASS.

Baseline drift review: `Cargo.lock`, manifests, dependency graph, ADR 0027,
and ADR 0029 are unchanged. HostExplicit remains exactly eleven names;
`repo.create-directory` remains ineligible. The Codex baseline remains
`0.149.0`. Only the three Rust files and this audit record are in scope.

## Final verdict

**PASS WITH NARROW HARDENING.** Relative linked gitfile spellings now resolve
under the same closed relationship checks; the Desktop linked admission test
uses production semantic validation; Stage/Unstage test hooks are root-bound;
and deterministic linked activation metadata-race evidence was added. No
material ADR 0029 contradiction remains. No new authority, dependency,
persistence field, selector, or production Git lifecycle command was added.

Windows deterministic tests and the local full-workspace gates pass. Linux
deterministic CI evidence and exact-head publication status are reported in
the task closure after the pushed head's CI completes.

## Live-only questions

Task 361 owns supported-host Windows live admission/switch/Close/removal
certification. This audit does not emit `RAH_V030_LINKED_WORKTREE_LIVE_OK`
and makes no GUI, model-selected linked-worktree dispatch, Linux live, macOS
live, or cross-platform live-parity claim.
