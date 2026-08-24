# Bounded multi-replacement `repo.patch` implementation

Date: 2026-08-24

## Scope

Extend the existing `repo.patch` authority with up to sixteen exact
replacements in one already-authorized tracked UTF-8 worktree file. Preserve
the legacy single-replacement request form and use one native final target
replacement after constructing a complete in-memory postimage.

## Implementation

1. Amend ADR 0012 without changing the authority class or capability identity.
2. Parse two closed mutually exclusive forms and normalize them privately to a
   replacement vector. Enforce 64 KiB request, item, and aggregate text bounds,
   a 16-item count bound, and the existing 1 MiB input/output bounds.
3. Resolve every exact occurrence in the original snapshot, reject duplicate or
   overlapping ranges, then construct final bytes in original-offset order.
4. Keep the existing same-directory temporary file, immediate pre-commit
   revalidation, one native replacement commit point, and exact postimage
   observation. Preserve Unix permission mode on the temporary target image.
5. Add deterministic coverage for multi-replacement construction, closed-form
   failures, fault outcomes, and Unix mode preservation.

## Non-goals

No profile, Generic Tool Bridge production, live Codex, multi-file, namespace,
Git history, unified-patch, or dependency changes are included.
