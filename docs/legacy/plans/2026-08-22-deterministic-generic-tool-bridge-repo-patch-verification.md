# Task 051: Deterministic Generic Tool Bridge verification for `repo.patch`

Status: Implemented
Date: 2026-08-22

## Scope

Task 051 verifies the existing provider-agnostic Generic Tool Bridge without a
Codex executable, model, credentials, or network connection. It does not change
`repo.patch`, trusted-profile authority, or Codex-owned capability restrictions.

## Deterministic chain

The primary bridge tests construct an owned temporary Git repository and use the
actual host path:

```text
trusted static profile
-> TrustedStaticProfile::load
-> rah_cli::profile_composition::compose
-> fresh effective ToolRegistry
-> CodexRuntime fake app-server transport / Generic Tool Bridge
-> canonical repo.patch
```

The static profile binds only symbolic `git` and `worktree` resources. Effective
composition is the sole constructor of `RepositoryWorktreePatchTool`; its
private `RepositoryWorktreeMutationPolicy` remains unexposed. No test manually
constructs the patch tool for composition-to-bridge coverage.

## Verified behavior

- Effective composition exposes canonical `repo.patch` with
  `PermissionLevel::Execute`. The bridge privately advertises deterministic
  `rah_tool_0` and resolves it back to the canonical ToolRegistry entry.
- `Execute` in the bridge allowlist permits dispatch. `None`, `Read`, and
  `Write` individually deny before tool execution; worktree bytes and index
  remain unchanged. Bridge configuration does not create or modify the private
  mutation policy: `PermissionLevel::Execute != RepositoryWorktreeMutationPolicy`.
- One valid call produces one ToolRequested, ToolStarted, ToolFinished, actual
  composed-tool invocation, model-visible response, and observed worktree
  replacement. The index and an unrelated tracked file remain unchanged.
- Duplicate delivery of one `(thread, turn, callId)` returns the bridge's stored
  response and does not execute the composed tool again. The bridge never
  issues an automatic retry.
- Representative stale digest, duplicate expected text, staged target, and
  invalid-path failures preserve the existing public failure representation;
  they are not retried. No native replacement occurs for the known
  precondition/refusal cases (the private native-attempt counter remains covered
  by Task 049's `rah-tools` tests).
- A test-private delegate gate, confined to `rah-runtime-codex` unit tests,
  proves cancellation before entry to the actual composed tool invokes it zero
  times. A gate after the real tool returns proves cancellation and disconnect
  after its mutation boundary do not replay it or roll back the postimage.
  The bridge cannot deterministically interrupt inside the private synchronous
  native replacement call without exposing Task 049's crate-private seam, so no
  rollback or new async mechanism was added.
- A test-private post-delegate uncertain-result fixture models lost certainty
  after one real composed invocation. The bridge translates that error once and
  duplicate delivery uses only the existing cached response; it never retries
  the external effect.
- The translated successful and refusal outputs are checked for absence of
  absolute repository/target/Git paths and example preimage/postimage content.
  The public output remains only the existing bounded JSON status fields.
- The bridge handshake remains restricted: Codex-owned shell, unified exec,
  filesystem, web, image, app, and MCP capabilities stay disabled. Existing
  mixed-provider composition tests continue to cover MCP/Process Plugin
  lifecycle ownership; Task 051 adds no provider-specific bridge path.

## Windows results and limits

The deterministic suite ran on Windows against temporary Git repositories and
the existing native Windows `repo.patch` implementation. It observed one
postimage replacement for the successful bridge call, a clean index, and no
unrelated-file change. Native attempt counting and forced uncertain replacement
outcomes remain intentionally crate-private Task 049 policy tests; Task 051
tests the bridge's exactly-once/no-replay handling around the composed tool.

## ADR status

ADR 0012 remains **Proposed**. This record neither accepts it nor broadens
worktree, index, process, filesystem, MCP, or Process Plugin authority.

## Suggested next task

Task 052: live Codex validation of trusted-profile-composed `repo.patch` with
the version-pinned `codex-cli 0.149.0`, one bounded mutation, exact single
execution, restricted Codex-owned capabilities disabled, terminal Completed,
and verified child/temporary cleanup.
