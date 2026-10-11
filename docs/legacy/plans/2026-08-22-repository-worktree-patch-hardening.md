# Task 049: `repo.patch` deterministic boundary audit and hardening

Status: Implemented
Date: 2026-08-22

## Scope

Task 049 audits Task 048's private, host-constructed
`RepositoryWorktreeMutationPolicy`. It does not compose a trusted profile, add
Codex, MCP, Process Plugin, or live integration, and does not broaden the
single-file literal-replacement authority.

## Audit result and hardening

The Task 048 validation ordering and shared per-repository lease remain
appropriate: the lease serializes RAH calls only, while root, `.git`, parent,
target, Git/index, preimage, and postimage observations are revalidated before
the one native replacement attempt.

This task corrected two concrete proof gaps:

1. A flushed temporary postimage is now represented by a private captured file
   identity. It is revalidated as a regular, non-link/non-reparse, single-link
   file with the exact constructed bytes immediately before the replacement
   attempt.
2. Success now requires the post-replacement target identity to equal that
   captured temporary identity, in addition to exact postimage bytes and
   unchanged repository observations. Same content at a newly substituted
   identity is uncertain, not success.

Temporary names now use a host-generated v4 UUID and exclusive creation rather
than a PID/counter-only name. The normal cleanup paths revalidate the temporary
identity and, where relevant, bytes first. If temporary evidence was tampered
with or disappeared, it is retained where possible and the result is
`uncertain`; it is not treated as a known replacement failure.

## Deterministic fixtures

Private unit-test hooks inject an external state change after these phases:

| Phase | Result exercised |
| --- | --- |
| initial path validation | target identity replacement refuses before commit |
| Git/index observation | staged target and HEAD change refuse before commit |
| preimage validation | changed preimage refuses before commit |
| unique-text validation | changed target refuses before commit |
| temporary write | changed target refuses; temporary-byte tampering is uncertain |
| final target identity check | repeated identity check refuses before commit |
| immediately before replacement | one injected failure permits exactly one attempt |
| immediately after replacement | same-byte, changed-identity target is uncertain |

Windows fixtures use a real `CreateFileW` handle that shares read/write but
denies delete. Releasing that handle at the pre-replacement phase permits the
native `MoveFileExW` attempt to succeed. Keeping it open produces a known
replacement failure only while the original target and temporary postimage can
both be proven intact. Removing temporary evidence after that failure returns
`uncertain`.

## Result classification after audit

- `precondition_failed`: before a replacement attempt, including stale target,
  Git/index/HEAD/ref, root/path, preimage, or temporary validation failures,
  when cleanup is proven.
- `replacement_failed_known`: the one native call returned an error, the root
  and target retain the original path identity and complete preimage, the
  temporary retains its captured identity and complete postimage, and cleanup
  succeeds.
- `ok`: the one native call succeeded; root/repository state remains valid;
  target parent/path safety remains valid; target identity equals the captured
  temporary identity; and target bytes equal the complete constructed
  postimage.
- `uncertain`: every other post-attempt or cleanup-ambiguous case, including
  missing/tampered temporary evidence, target identity/content/path changes,
  failed post-observation, or repository-state change. No result class retries,
  restores, or replays the replacement.

The native replacement invocation remains the internal commit point. A future
cancellation adapter can distinguish before the invocation, the invocation in
progress, and after its return. It must classify cancellation at or after that
point as uncertain unless the same post-state proof above has already been
recorded; Task 049 adds no cancellation API or rollback.

## ADR review

ADR 0012 remains **Proposed**. The deterministic Windows baseline now has
stronger evidence, but the ADR itself reserves acceptance until the broader
release/milestone decision is made. This task deliberately has no trusted
profile composition, Codex bridge, or live integration evidence and therefore
does not change the ADR's status merely because tests pass.

## Suggested next task

Task 050 may design and implement trusted-profile composition for the hardened
`repo.patch` constructor, without Codex live validation.
