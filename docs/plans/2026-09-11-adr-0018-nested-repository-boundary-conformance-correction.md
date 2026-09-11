# Task 294 — ADR 0018 Nested Repository Boundary Conformance Correction

Status: implementation plan for the focused ordinary-authority correction.

## Scope

Harden the existing `RepositoryFileRenamePolicy` so the ordinary
`repo.rename-file` operation proves that source and destination ancestry stay
outside nested repository boundaries. The correction preserves the existing
Tool schema, permission, statuses, one-native-attempt boundary, and authority
model. It does not create or accept ADR 0026 and does not change HostExplicit,
Desktop, frontend, provider, Trusted Profile, or release behavior.

## Proof and evidence

The rename-local ancestry proof will inspect every logical directory component
from the selected canonical root to both source and destination parents. The
selected root's own `.git` metadata remains the outer repository boundary; any
`.git` entry below that root, including a directory, gitfile, or contradictory
observation, is a nested-boundary refusal. Capture, immediate revalidation,
and post-effect proof use the same check.

The focused deterministic evidence will cover:

1. an existing nested repository in destination ancestry;
2. an outer-tracked source whose ancestry is a nested repository;
3. a nested boundary appearing between capture and immediate revalidation;
4. a contradictory nested boundary appearing after a possible native effect;
5. the existing no-replay and exact public-result behavior on all outcomes.

## Mount-like ancestry audit

ADR 0018's mount-like wording is retained only where the implementation proves
it. The Linux native rename path will use the authority-neutral `/proc/self/mountinfo`
observation to reject mount points below the selected repository root and will
fail closed when that observation is unavailable or malformed. The existing
Unix device identity comparison remains a same-filesystem check, not a claim
that it alone detects same-device bind mounts. Windows uses the existing
reparse-point and volume-identity checks. Non-Linux Unix already has no native
rename primitive and remains fail-closed without a fallback.

If the audit exposes a boundary that cannot be resolved by this narrow,
authority-neutral ancestry proof, implementation stops and the unresolved gap
is reported rather than broadening the authority.

## Validation and closure

Run the focused rename tests and the normal deterministic workspace gates,
recording Unix and Windows availability separately. Verify unchanged public
Tool contract and changed-file scope, commit the focused correction, push it,
and require successful CI for the exact pushed commit before the final ADR 0018
acceptance re-audit and Task 294 closure. ADR 0026 remains out of scope.
