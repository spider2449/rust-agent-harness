# Task 138: repository commit Generic Tool Bridge verification

Baseline: Task 137 commit `e02cd3b6ebef789531c47b856e841a9df8e8b05f` and
exact-head CI `33284958371` PASS.

## Deterministic path

The Task 138 integration fixtures use a task-owned disposable Git repository
and the real composition path:

`TrustedStaticProfile -> rah_cli::profile_composition::compose ->
EffectiveProfileComposition::registry_handle -> Generic Tool Bridge`.

The separate `EffectiveProfileComposition::repository_commit_control()` is the
only host-side route used to call `authorize_current_reviewed_snapshot()`. It
is not registered as a Tool, is not in the dynamic snapshot, and composing or
advertising a profile does not arm it.

## Bridge contract covered

`repo.commit` remains the public RAH ToolName with Execute permission and its
exact object schema has only required `message`, string `maxLength: 16384`, and
`additionalProperties: false`. The bridge makes a Codex-compatible private
`rah_tool_N` alias; the tests discover it from the snapshot rather than relying
on a fixed suffix. The public dotted name is never advertised as a substitute
alias. The model receives neither review authorization nor repository, HEAD,
index, tree, Git executable, argv, identity, credential, branch, or ref fields.

The integration test sends real `item/tool/call` requests. Execute permission
is an outer bridge gate: an unarmed call finishes with the bounded
`precondition_failed` ToolOutput, while a permission-denied call does not start
the Tool and does not consume a previously armed host authorization. A later
Execute-enabled bridge can use that same still-current authorization exactly
once.

Malformed/extra/missing/wrong-type/over-16-KiB messages are translated as
bounded `invalid_input`; Task 137's retention rule remains observable through
the bridge because a subsequent valid message commits the originally armed
snapshot. A valid unarmed call does not auto-review. A stale authorization is
consumed by its first valid evaluation, and restoring the former staged state
cannot revive it.

The verified commit is inspected from Git rather than trusted from the dynamic
response: it advances the attached branch once, has the prior HEAD as sole
parent, retains the provided message, uses the trusted host identity, has no
signature header, and its response `commit_oid` equals actual HEAD.

Completed identical `(threadId, turnId, callId, alias, arguments)` replay
returns the cached dynamic response and does not create another commit.
Conflicting call-ID replay is rejected. A fresh call ID is distinct from replay,
but the consumed host authorization causes it to fail closed. Existing generic
bridge tests cover in-flight duplicates, wrong thread/turn, unadvertised alias,
namespace rejection, definition snapshot mismatch, cancellation, disconnect,
terminal cleanup, alias collision, and bounded-call-cache behavior. Their
guarantee composes with Task 136/137's consumed authorization: eviction or
cancellation can never recreate commit authority, and uncertain effects are
not retried.

## Coverage split and limits

Task 138 exercises `invalid_input`, `precondition_failed`, and
`committed_verified` through the real composed bridge. Task 136 covers private
`known_no_effect`/`uncertain` policy dispositions and Task 137 covers pending
authorization consumption and retention. Generic Tool Bridge translation and
cancellation/replay fixtures are shared bridge coverage; no invasive commit
fault hook or production authority seam was added.

This is deterministic Windows/Linux fixture coverage only. It makes no Codex
app-server, model, network, Desktop, release, or certified-live claim. Task 139
remains deferred pending exact-head CI.

## Authority conclusion

ADR 0016 remains authoritative. No production code, public API, trusted-profile
schema, dependency, version, or Desktop change is required. The bridge is only
a private routing adapter and does not become commit authorization.
