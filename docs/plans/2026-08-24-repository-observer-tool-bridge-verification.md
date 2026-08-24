# Task 064: deterministic repository observer Generic Tool Bridge verification

Status: Implemented
Date: 2026-08-24

## Deterministic chain

The bridge tests use no Codex executable, credentials, network connection, MCP
server, or native Codex capability. Each primary test builds a temporary Git
repository and follows the host-owned path:

```text
TrustedStaticProfile::load
-> rah_cli::profile_composition::compose
-> fresh effective ToolRegistry
-> CodexRuntime fake app-server / Generic Tool Bridge
-> real repository observer
```

The composed profile contains `repo.file-info`, `repo.status`, `repo.diff`, and
`repo.diff-staged`, with an additional `repo.patch` only in the routing-isolation
case. Definitions retain `PermissionLevel::Execute`; no observer executes while
the profile is composed.

## Alias, schema, and authority evidence

The fake app-server receives the exact `ToolDefinition` input schema from the
effective registry. The bridge assigns deterministic private `rah_tool_<n>`
aliases and retains the canonical RAH name for every lifecycle event and
registry lookup. Registry ordering is deliberately not treated as a public
numeric-alias contract. Canonical names and unknown aliases are rejected as
app-server tool names, with no fallback to shell, generic Git, `repo.patch`, or
another observer.

With `repo.patch` composed beside all observers, each remains a separate
canonical registry route under the same outer Execute permission. Execute
admission does not collapse the per-tool authority boundary.

## Execution and read-only evidence

The temporary repository contains an unstaged tracked edit, a staged edit, and
an untracked file. Through the bridge, the suite verifies normalized successful
results for all four observers:

- `repo.file-info` reports the repository-relative tracked path.
- `repo.status` reports all three representative state classes.
- `repo.diff` reports `worktree_to_index` / `index` and excludes the staged-only
  entry.
- `repo.diff-staged` reports `index_to_head` / `head` and excludes the further
  unstaged entry.

Before and after each representative call, tests capture HEAD, refs, index,
the tracked worktree byte content, and untracked byte content. They remain
unchanged. This demonstrates no intentional repository mutation; it does not
claim Git makes zero incidental filesystem writes.

The binary diff case reports `binary = true` and `patch = null` without a
binary payload. Existing observer unit coverage continues to provide the Unix
invalid-UTF-8 tagged-base64 parsing evidence; Task 064's Windows-friendly bridge
fixture exercises UTF-8 tagged paths end to end.

## Permission, failure, and lifecycle evidence

For each observer, `None`, `Read`, and `Write` are denied before the delegated
observer enters. Execute admits dispatch. Test-only wrappers confined to the
runtime-codex test module prove denied calls enter zero times, duplicate call
identities enter once, and a new call ID enters again. Duplicates receive the
bridge cached response, so the bridge does not rerun the observer's Git command
sequence.

Invalid observer inputs are forwarded through the real observer validation
path, produce the existing bounded bridge error, execute once, do not retry,
and leave the snapshot unchanged. The representative success calls emit the
existing ordered ToolRequested, ToolStarted, and ToolFinished lifecycle.

A deterministic gate around real composed `repo.diff` proves cancellation
before observer entry produces no observer execution and no replay. The generic
bridge's existing post-return cancellation and disconnect tests remain the
applicable behavior for an observer after its synchronous read-only command
sequence: no rollback is invented and no call is replayed after uncertain
delivery. Responses are checked not to expose repository or Git executable
paths, HOME, profile paths, host-policy names, or observer internals.

## Scope and ADR status

There are no production bridge changes, no new dependencies, no public API
changes, no profile-schema change, and no ADR change. Codex-owned shell,
filesystem write, MCP, process, network/web, image, and approval capabilities
remain disabled in the fake bridge handshake.

## Suggested next task

Task 065: live Codex validation of the trusted-profile repository observer
toolkit using exactly `codex-cli 0.149.0`, with a read-only multi-observer
workflow and explicit repository snapshot assertions.
