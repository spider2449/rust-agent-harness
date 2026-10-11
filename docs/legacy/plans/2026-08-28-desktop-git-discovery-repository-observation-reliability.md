# Task 123: Desktop Git Discovery and Repository Observation Reliability

## Phase 1 scope

Diagnose the Desktop-only Git discovery and repository observation failure without
changing repository authority, Git command shapes, or the frontend diagnostic
surface.

1. Audit the explicit Git override seam and document a closed Windows resolver
   design that preserves native executable identity validation and avoids PATH
   command execution.
2. Add a Desktop-private observation-stage classifier for status execution or
   revalidation, worktree diff execution, staged diff execution, and normalized
   output parsing. The public frontend result remains sanitized.
3. Exercise real native Git against independent temporary repository states and
   repository replacement transitions, including generation and tool-instance
   replacement assertions.
4. Stop after the failing layer is identified or after reporting that the live
   condition could not be reproduced from the available deterministic inputs.

No commit is authorized in this diagnostic phase.

## Phase 2 scope

For the already host-selected and canonicalized repository identity only, add
one exact `safe.directory` value to the private observer Git environment using
Git's counted configuration environment. Preserve the isolated system/global
configuration and all fixed execution and identity controls. Do not add
executable discovery, generic Git configuration, wildcard trust, or any
frontend diagnostic disclosure. Cover ordinary repository observation,
foreign-owner simulation, per-repository replacement, identity revalidation,
and deterministic environment contents.
