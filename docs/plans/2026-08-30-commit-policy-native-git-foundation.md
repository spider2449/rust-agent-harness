# Task 135 — Commit policy / native Git foundation

ADR 0016 remains the authority for this private foundation. The implementation
lives in `rah-tools::repository_commit` and deliberately exports neither a Tool
nor a generic Git API.

`RepositoryCommitPolicy` reuses `RepositoryIdentity`, the exact native process
policy, and the canonical-root shared mutation lease already used by stage,
unstage, and repository mutation tools. It owns host identity, a host-created
empty temporary hooks directory, a cleared/rebuilt Git environment, fixed
configuration, and a fixed normal-commit command. The child environment disables
system/global configuration, terminal prompts, inherited ambient variables, and
uses only host-created numbered configuration entries.

`ReviewedCommitAuthorization` is private, in-memory, non-serializable, and
consumed by value. It binds the attached branch, old HEAD, raw index SHA-256,
canonical `ls-files --stage -z --no-abbrev` SHA-256, and `write-tree` OID.
Capture and pre-spawn revalidation reject ordinary unsupported repository,
branch, special-state, sparse, conflict, gitlink, and index conditions. The
authorized `write-tree` observation may create unreachable tree objects as
allowed by ADR 0016.

The only mutating command shape is host-fixed `git -c ... commit --no-verify
--cleanup=verbatim -m <validated-message>`. It has bounded output/timeout and
one attempt per consumed authorization. Post-observation returns one private
taxonomy: invalid input, precondition failed, known no effect, committed
verified, or uncertain. Verified success checks branch/HEAD, parent, tree,
verbatim message, host author/committer, no signature, and post-commit index
tree semantics.

Focused disposable-repository tests cover a normal staged commit, dirty and
staged-plus-unstaged worktree preservation, invalid input and snapshot-change
refusal, hook neutralization on Unix fixtures, and command-scoped signing
configuration override. Task 136 remains responsible for the exhaustive race,
platform, malformed-index, cancellation, and Linux-specific hardening matrix.

No Tool, trusted-profile schema, Generic Tool Bridge, Desktop behavior,
persistence, dependency, version, tag, or release is introduced by this task.
