# Task 136 - Deterministic repository commit hardening

## Starting state

Task 135 provided a private `RepositoryCommitPolicy` at
`8497a1d55395b2f6bbe5cc0d6c1319b7e84114fc`. It exposed no `repo.commit` Tool,
profile composition, runtime binding, or Desktop behavior. ADR 0016 remains
authoritative.

## Security hardening

- Capture an immutable native executable policy at construction and revalidate
  that exact identity before every fixed child command.
- Bind the host-owned empty hooks directory to native filesystem identity as
  well as canonical path and emptiness; replacement at the same path fails
  closed.
- Bind each in-memory, non-Clone authorization to a private UUID policy
  generation. A different policy instance cannot consume it.
- Preserve a single consuming commit path. Test accounting records attempted
  mutating spawns only after final revalidation; no path retries the command.
- Harden success observation: verify `cat-file -t` is `commit`, require one
  tree/author/committer header, one expected parent, no `gpgsig`, exact
  verbatim message behavior, host identity, and post-index tree.

## Test-only seams

`#[cfg(test)]` contains a finite, process-local phase seam: after
authorization, after lease, before final revalidation, before spawn, after
spawn, before post-observation, during committed verification, and during
known-no-effect verification. It can only inject a bounded observation failure,
synthetic spawn failure, or a test-owned index lock. It is absent from release
builds and is not a callback/plugin API.

## Evidence matrix and semantics

Focused disposable repositories cover reviewed-index mismatch, symbolic-HEAD
change, cross-policy authorization rejection, hostile local hooks/signing/
identity/editor/template configuration, fixed argv exclusions, message limits,
spawn failure, actual one-attempt index-lock refusal, and post-spawn observer
failure. Existing foundation coverage retains normal dirty-worktree commits,
special-state refusal, malformed/unsupported admission, and private hooks
neutralization.

`known_no_effect` is intentionally limited to fresh proof of the same
repository identity, same attached branch, `HEAD == old_head`, and authorized
branch `== old_head`; it does not assert absence of unreachable objects or
incidental index/cache writes. Timeout, cancellation, output loss, observer
failure, or any incomplete postcondition is `uncertain` unless that independent
no-effect proof succeeds. A branch/index race before spawn is
`precondition_failed` with zero attempts.

The fixed environment clears inherited Git, identity, editor, pager, helper,
home/XDG, proxy, and configuration variables, then supplies only host-owned
configuration including disabled system/global config, empty hooks path,
disabled signing, explicit identity, and safe directory. Repository-local
presentation settings remain ambient only where Git cannot affect this fixed
command; security-relevant hook, signing, and identity keys are overridden.

## Platform and deferrals

The common tests are Windows/Linux compatible. Unix-only hook script and
executable-bit coverage remain platform-gated; Windows retains native `.exe`,
canonical path, and Job Object supervision through existing host execution.
Linked worktrees, bare repositories, sparse/alternate indexes, gitlinks,
special merge/rebase states, remote Git, signing, and all model-facing
composition remain refused/deferred. Task 137 may compose this private,
generation-bound authority only after this deterministic evidence is accepted.
