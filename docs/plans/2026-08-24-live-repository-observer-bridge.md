# Task 065: live trusted repository observer bridge validation

Status: Implemented
Date: 2026-08-24
Platform: Windows
Codex baseline: `codex-cli 0.149.0`

## Live chain and fixture

The opt-in example `live_trusted_profile_repository_observers_bridge` verified
the complete host-owned chain:

```text
TrustedStaticProfile::load
-> rah_cli::profile_composition::compose
-> fresh effective ToolRegistry
-> Generic Tool Bridge
-> native Codex app-server
-> repository observer calls
-> Completed
```

Each live run creates and removes a fresh native Git fixture outside the RAH
checkout. Its committed baseline has `tracked.txt` and `staged.txt`; the live
state then has an unstaged `tracked.txt` change, a staged `staged.txt` change,
and untracked `untracked.txt`.

The trusted profile uses one symbolic native Git executable and one symbolic
temporary repository. It contains exactly these Execute capabilities, with no
`repo.patch` capability:

- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`

## Live evidence

The hardened app-server executable discovery resolved the native executable and
required its exact version output: `codex-cli 0.149.0`. Three fresh fixture
runs passed. In every run the effective registry contained only the four
observers, all with `PermissionLevel::Execute`, and private bridge aliases were:

| Canonical observer | Private alias |
| --- | --- |
| `repo.diff` | `rah_tool_0` |
| `repo.diff-staged` | `rah_tool_1` |
| `repo.file-info` | `rah_tool_2` |
| `repo.status` | `rah_tool_3` |

The numeric aliases are a per-thread implementation detail, not a public
contract. All four aliases were distinct. Every run recorded exactly one
`ToolRequested`, `ToolStarted`, `ToolFinished`, and underlying invocation for
each canonical observer. No duplicate call identity, bridge replay, retry, or
`repo.patch` request occurred.

The runtime event order was `Started`, `ModelRequestStarted`, the four ordered
observer lifecycles, model deltas, and `Completed`. The final assistant output
was exactly `RAH_REPOSITORY_OBSERVERS_LIVE_OK` in all three runs.

## Semantic, isolation, and read-only evidence

- `repo.status` reported the unstaged tracked edit, the staged edit, and the
  untracked file.
- `repo.file-info` queried `tracked.txt`, reported it tracked, and reported its
  worktree modification relative to the index.
- `repo.diff` reported `worktree_to_index`, base `index`, included `tracked.txt`,
  and excluded the staged-only `staged.txt` delta.
- `repo.diff-staged` reported `index_to_head`, base `head`, included `staged.txt`,
  and excluded `tracked.txt`.

Before each live run the example captures HEAD, refs, raw index bytes,
`tracked.txt`, `staged.txt`, and `untracked.txt` bytes, plus the staged and
unstaged semantic diffs. All values remained unchanged after `Completed`.

The bridge-mode handshake disabled Codex-owned shell, file access and mutation,
MCP, process execution, network/web, image, apps/connectors, and approvals.
The model could inspect the fixture only through the RAH observer tools. The
example audited model-visible tool output and final response for fixture and
Git executable paths, profile identity/path evidence, policy and observer
internals, and unbounded host diagnostics; none were exposed. Repository
relative names and bounded diff snippets remain expected observer output.

Each run explicitly shut down and reaped the Codex app-server, observed no
remaining Git/MCP/Process Plugin child, removed the profile and fixture, and
confirmed temporary repository removal.

## Validation scope

Task 065 changed no production code, dependencies, public APIs, trusted-profile
schema, Generic Tool Bridge semantics, permissions, or ADRs. It adds only the
opt-in live example and this evidence record.

Windows live observer validation passed with `codex-cli 0.149.0`. Ubuntu
deterministic CI run `32682605820` at
`ea3657596ba653a6398f5928a8e27b319a9ab839` completed successfully. This is
cross-platform deterministic implementation evidence only; no Unix live Codex
validation is claimed.

The optional unborn-HEAD live probe was not run because Task 062 deterministic
coverage already proves the `base = empty_tree` contract and it is not required
for the primary four-observer gate.

## Suggested next task

Task 066 — RAH v0.6 Repository Observer Milestone Audit. Audit Tasks 057–065
for scope/authority, observer contracts, deterministic and live evidence,
security boundaries, and known limitations; decide release readiness without
bumping a version or creating a tag.
