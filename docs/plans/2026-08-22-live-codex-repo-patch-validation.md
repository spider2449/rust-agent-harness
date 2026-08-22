# Task 052: Live Codex validation for trusted-profile-composed `repo.patch`

Status: Passed
Date: 2026-08-22

## Scope and command

This is the first opt-in live validation of the already-hardened, trusted
profile-composed `repo.patch` path. It adds no mutation semantics, authority,
fallback tool, or profile-composer path.

The focused live command was:

```powershell
cargo run -p rah-runtime-codex --example live_trusted_profile_repo_patch_bridge
```

It passed using exactly `codex-cli 0.149.0`. Before the run, `codex --version`
reported that exact string. The adapter was passed `codex` and applied its
existing Windows executable-discovery rule, which resolves and canonicalizes a
native `.exe` (not a `.cmd` or `.ps1` launcher), then reruns the exact version
and experimental app-server schema checks before spawning `app-server --stdio`.
The native executable identity is intentionally redacted from this record.

## Actual composition and authority boundary

The example uses the product path exactly:

```text
trusted static profile
-> TrustedStaticProfile::load
-> rah_cli::profile_composition::compose
-> fresh effective ToolRegistry
-> Generic Codex Tool Bridge
-> repo.patch
```

It does not construct `RepositoryWorktreePatchTool` directly and it does not
add a second profile composer. The trusted static profile used only the redacted
symbolic resources `live-git` (native Git executable) and `live-repository`
(the isolated Git worktree), with one enabled `repo.patch` capability at
`PermissionLevel::Execute`.

The effective inventory and fresh registry contained exactly one canonical RAH
tool, `repo.patch`, with `Execute`. The bridge's deterministic private alias was
`rah_tool_0`; it was recorded as `rah_tool_0 -> repo.patch`. The advertised
schema was the existing closed five-field schema:

```text
path
expected_file_sha256
expected_file_byte_length
expected_old_text
replacement_text
```

`Execute` was the bridge dispatch allowlist only. The actual mutation authority
remained the private `RepositoryWorktreeMutationPolicy` plus the deterministic
`repo.patch` eligibility checks; neither the profile nor the model request
created standalone mutation authority.

## Fixture and request

The host created a fresh temporary Git repository, committed a clean tracked
stage-0 target and an unrelated tracked file, then generated the trusted profile
outside that worktree. The target began as the harmless strict-UTF-8 regular
file text `RAH_LIVE_PATCH_BEFORE\n` (22 bytes), with SHA-256:

```text
dfea2b072cbc8d4280532505bd673d4b6d78f3213c5b8377083ee7883d1ff14e
```

The one supplied request used logical path `target.txt`, that SHA-256 and byte
length, the unique literal `RAH_LIVE_PATCH_BEFORE`, and replacement text
`RAH_LIVE_PATCH_AFTER`. The prompt directed Codex to invoke the one advertised
RAH patch tool exactly once and, after success, return exactly
`RAH_REPO_PATCH_LIVE_OK`.

## Live result

The terminal result was `Completed`. The observed event sequence was:

```text
Started
-> ModelRequestStarted
-> ToolRequested
-> ToolStarted
-> ToolFinished
-> ModelDelta (eight events)
-> Completed
```

The final model output was exactly `RAH_REPO_PATCH_LIVE_OK`. Counts were all
one:

```text
ToolRequested                  1
ToolStarted                    1
ToolFinished                   1
actual repo.patch invocation   1
native replacement attempt     1
```

The native-attempt value comes from a default-disabled,
`live-test-support` process-local fixture counter at the existing replacement
commit point. It is unavailable from the normal `rah-tools` crate build, is not
model-visible, and cannot alter policy, schema, input, permissions, or
execution behavior.

The expected postimage existed, the preimage was absent, the index bytes, HEAD,
and refs were unchanged, the unrelated file was unchanged, and porcelain status
was exactly the ordinary worktree-only ` M target.txt` result. No
`.rah-repo-patch-*` sibling remained.

## Restrictions, privacy, and cleanup

The bridge remained restricted: Codex-owned shell, filesystem, MCP, arbitrary
process, network-tool, web, image, app, and approval capabilities were all
disabled. The only mutation path was:

```text
Codex -> Generic RAH Tool Bridge -> repo.patch
```

The model-visible successful tool response was the existing bounded status JSON
only. The fixture audited it for absence of host/repository paths, temporary
names, Git executable identity, and fixture preimage/postimage text. The final
model output was only the required marker.

After the terminal event, the owned Codex app-server shutdown/reap completed
successfully. This profile has no MCP or process-plugin provider, so no such
child existed. The fixture verified no patch sibling files, removed its entire
temporary repository successfully, and relies on the owned runtime lifecycle
rather than global process-name polling.

## Compatibility and ADR state

No app-server schema or lifecycle difference from the pinned baseline was
observed. No live-only production defect was found. ADR 0012 remains
**Proposed**: one successful live validation does not accept it, and this task
does not authorize restore-worktree, generic filesystem/shell/process access,
MCP/network capabilities, multi-file patching, or Git history/ref mutation.

## Suggested next task

Task 053 — v0.5 repository-mutation milestone audit: assess ADR 0012 acceptance
readiness, verify all deterministic/profile/bridge/live evidence against the
intended authority boundary, and identify remaining release blockers before
expanding scope.
