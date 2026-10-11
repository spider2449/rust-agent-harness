# Task 296 — ADR 0018 Git Metadata Revalidation Conformance Correction

Status: focused implementation plan for the ordinary `repo.rename-file`
authority only.

## Scope

Close the persistent same-target `.git` link/reparse substitution gap in the
existing `RepositoryFileRenamePolicy`. Preserve the public Tool name,
permission, four-field request, bounds, result shapes, and one-native-attempt
boundary. Do not create or accept ADR 0026 and do not change HostExplicit,
Desktop, frontend, providers, Trusted Profile, Tauri permissions, or release
state.

## Correction

Use the existing host-owned `reject_link_or_reparse` helper together with an
explicit directory-kind check for `<canonical-root>/.git` during construction
and every `repository_ok()` validation. Compare the followed `FileIdentity`
only after the supported path-entry form has independently passed.

The existing check-and-revalidate model remains best-effort against arbitrary
external races. This correction closes the deterministic persistent
same-target substitution gap; it does not claim a handle-bound or race-free
filesystem proof.

## Deterministic evidence

Extend the existing production rename test hook with actual platform fixtures:

1. stable ordinary `.git` directory succeeds;
2. `.git` removed or replaced by a different ordinary directory is rejected;
3. linked-worktree/gitfile form remains unsupported;
4. same-target Unix symlink or Windows junction before initial execution is
   `precondition_failed` with zero native attempts;
5. same-target link/reparse substitution between capture and revalidation is
   `precondition_failed` with zero native attempts;
6. same-target link/reparse substitution after the possible native effect is
   `uncertain` with exactly one attempt and no replay or compensation.

The ordinary ADR 0018 audit will re-check repository/root/metadata/Git
identity, ancestry and nested boundaries, mount observation, Git/index
equality, destination and parent proofs, alias/case handling, same-volume
semantics, one-at-most-one effect, known-no-effect binding, independent
post-effect proof, and uncertain classification.

## Validation and closure

Run focused rename tests, applicable Unix/Windows deterministic coverage, the
workspace format/check/test/clippy gates, and `git diff --check`. Verify the
changed-file scope and clean state, then commit and push the focused correction
to `master`; require successful CI for the exact pushed SHA before closure.
