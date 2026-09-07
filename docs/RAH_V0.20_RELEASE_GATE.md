# RAH v0.20.0 Release Gate

## Status

**RELEASE PREPARATION — NOT TAGGED OR PUBLISHED**

v0.20.0 is a prepared release candidate. v0.19.0 remains the current
immutable published release until Task 243 performs publication.

## Release identity

Before Task 243, the release identity is:

- Task 241 audit commit: `619bc3c0d8923a5306e3f0fd4cbccc3203a1ef0f`.
- Task 241 audit CI: `34088511150` PASS.
- Task 241 closure: `91b676bdd408235af0612749e1b98d9f1ac1688d`.
- Task 241 closure CI: `34088699720` PASS.
- v0.20.0 tag: absent.
- GitHub Release: absent.
- Publication: Task 243.

The Task 242 preparation commit is deliberately recorded only after it exists
and its exact-head CI passes. No tag object, GitHub Release ID, or tag CI run
is invented here.

## Task 241 verdict

**VERDICT B — MILESTONE COMPLETE WITH DOCUMENTED LIMITATIONS — RELEASE
PREPARATION MAY BEGIN.**

This gate preserves the Task 241 distinction. It does not convert Verdict B
into an unqualified full-live-certification claim.

## Product contract

RAH v0.20 adds an explicit Desktop Host Tool invocation workflow for a closed
first-party Tool set. HostExplicit dispatch reuses the current connected
Desktop composition and the shared D2 current-definition/permission gate; it
does not create new underlying capability authority.

The product claim is an explicit human Host action that does not depend on
model Tool selection. It is not a claim that Codex always selects Tools.

## ADR 0021 dispatch boundary

```text
host explicit dispatch != runtime/model dynamic dispatch != capability authorization
```

HostExplicit does not create repository, filesystem, branch/ref, commit,
provider, process, network, or generic authority. `PermissionLevel` is only a
dispatch category. Execute is not generic repository authority. Frontend state,
Tool visibility, Effective Authority, and human confirmation are not
capability authority. Model text can never automatically trigger HostExplicit.

## D2 shared authorization

D2 requires the call name to match the expected Tool definition, re-resolves
the current Tool from the current ToolRegistry, requires exact complete
ToolDefinition equality (name, description, input schema, and permission), and
requires current PermissionLevel to be explicitly present in the host allowed
policy. There is no permission hierarchy. Rejection executes zero Tools;
successful admission executes exactly one Tool with no retry.

D2 does not own HostExplicit eligibility, capability authority,
lifecycle/provenance, repository outcome interpretation, replay, or rollback.

## Codex bridge hardening

The Codex bridge preserves thread ownership, active-turn ownership, private
alias, replay/deduplication, cancellation, response translation, and real
AgentEvent lifecycle. It calls `authorize_tool_dispatch` before `ToolStarted`
and `authorized_tool_dispatch` revalidates before execution. Captured
permission differing from current permission is a stale ToolDefinition even if
the replacement would otherwise be allowed. This aligns admission; it does
not make model Tool selection more reliable.

## Desktop connected-current composition

HostExplicit requires the actual current published Desktop composition. The
current registry, expected definitions, shared allowed permission policy,
repository/model/profile/connection generations, and selected repository/
context remain bound. Stale or disconnected state fails closed. There is no
silent reconnect, silent recompose, automatic provider activation, automatic
Trusted Profile restore, or disconnected execution route.

## First-release HostExplicit eligibility

Exactly these six first-party Tools are eligible:

- `fs.read`
- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`
- `repo.create-branch`

`repo.commit`, `repo.patch`, `repo.create-file`, `repo.edit-files`,
`repo.delete-file`, `repo.rename-file`, `repo.create-directory`, MCP Tools,
Process Plugin Tools, `echo`, fixtures, and generic host diagnostic Tools are
not eligible. These remain deferred by accepted scope.

## Typed input boundary

There is no generic `ToolName + arbitrary JSON` route and no production JSON
console. The exact first-release forms are:

- `fs.read`: `path`.
- `repo.file-info`: `path`.
- `repo.status`: `{}`.
- `repo.diff`: `{}`.
- `repo.diff-staged`: `{}`.
- `repo.create-branch` Prepare: `name`.
- `repo.create-branch` Confirm: ticket ID only.

The underlying Tool parser and policy remain authoritative.

## Branch prepare/review/confirm

Prepare performs no Tool execution and no Git branch effect. After sanitized
review, Confirm consumes a process-local, in-memory, non-persistent,
single-use, Tool-bound, input-bound, composition/currentness-bound ticket with
a five-minute TTL. Confirm accepts only the ticket ID, revalidates current
composition, does not auto-reprepare, and does not retry. D2 then reaches
existing ADR 0020 branch authority.

Verified success remains one absent `refs/heads/<name>` created at captured
committed attached `HEAD`, without switching branches.

## Host/model concurrency

The conservative first release allows one HostExplicit invocation maximum. No
HostExplicit action occurs during a model turn, and no model turn occurs during
`HostPrepared` or `HostRunning`. Read-only host actions participate. There is
no wait queue and no automatic retry. Conceptual coordinator states are
`Idle`, `ModelTurn`, `HostPrepared`, and `HostRunning`.

## Provenance

Desktop lifecycle provenance is `host_explicit`. Host activity is not called
model `ToolRequested`, `ToolStarted`, or `ToolFinished`. No AgentEvent schema
change was introduced. Task 240 observed `Started -> tool_completed` for
`repo.status` and `prepared -> Started -> tool_completed` for
`repo.create-branch`; each Tool executed exactly once.

## Cancellation/replay/crash semantics

Before start, prepared branch work may be cancelled with known no Tool effect.
After start there is no active HostExplicit abort operation; the backend owns
execution to terminal handling where possible. UI dismissal is not abort. No
retry, replay, compensation, rollback, ticket persistence, capability
persistence, automatic resume, or automatic conversation continuation exists.
A crash or lost result does not imply no effect.

## Conversation boundary

HostExplicit creates no user or assistant chat message, injects no ToolOutput
into model context, creates no fake model Tool call, and does not automatically
continue a model turn. Host activity remains separate Desktop state.

## Effective Authority / frontend

Effective Authority may expose only backend-owned `eligible`, `kind`, and
bounded unavailable reason. This is observational. The frontend does not
authorize or infer eligibility from names, permissions, effect classes,
authority categories, or advertised state. Tool text is text-safe; JSON is
escaped/preformatted; raw HTML ToolOutput rendering is not used.

## Tauri permissions

The narrow HostExplicit permission surface is:

- `host_invoke_read`
- `host_prepare_repo_create_branch`
- `host_confirm_tool_invocation`
- `host_cancel_tool_invocation`

There is no wildcard, generic D2 Tauri endpoint, or invoke-any-tool permission.

## Deterministic evidence

Task 239 deterministic evidence covers six-Tool eligibility, typed input,
currentness, D2 definition and permission changes, ticket lifecycle,
coordinator exclusion, provenance, strict branch-result parsing, safe output,
and narrow Tauri permissions. The required release validation must rerun these
normal deterministic gates but must not rerun live certification.

## Windows live certification

Task 240 is immutable live evidence. The certified production path is Windows
Desktop connected-current explicit Host Tool invocation for `repo.status` and
`repo.create-branch`, with no model request or model Tool lifecycle.

Certified environment: Windows NT `10.0.19045.0`, Git `2.54.0.windows.1`,
Codex `0.149.0`, executable SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, reasoning `medium`.

The live branch was `rah-host-explicit-live-18d2efa09d330900-2` at OID
`e6b376c26b0d97e12c1be2c7981aecc32974e29c`. Prepare had zero effect; Confirm
executed exactly once; result was `branch_created_verified`, `uncertain` was
false, the exact OID and ADR 0020 reflog were verified, no checkout/switch or
refresh occurred, generations were unchanged, and connection remained
Current. MCP and Process Plugin providers were both 0.

The live model counters were all zero: `runtime.start`, `AgentRequest`, prompt,
`ToolRequested`, `ToolStarted`, and `ToolFinished`.

## Live coverage limitations

Only `repo.status` and `repo.create-branch` were dedicated Windows live
certifications. `fs.read`, `repo.file-info`, `repo.diff`, and `repo.diff-staged`
are deterministically verified but not separately live certified. Real GUI
mouse-click automation was not certified. Linux and macOS HostExplicit live
behavior was not established. These are documented limitations, not blocking
gaps under Task 241 Verdict B.

## Task 229 limitation

Historical model-selected `repo.create-branch` dispatch remains not certified.
Two bounded attempts each recorded `ToolRequested = 0`, `ToolStarted = 0`, and
`ToolFinished = 0`. Task 240 does not supersede or reinterpret this evidence.

## Task 207 limitation

Model-selected MCP and Process Plugin Tool execution remains unestablished.
Task 207 is unchanged, and external provider Tools are not HostExplicit
eligible in v0.20.

## Platform limitations

Windows connected-current live behavior is established only for the two Tools
named above. Linux/macOS live behavior and real GUI automation are not claimed.

## Security nonclaims

Process supervision is not OS sandboxing. RAH does not claim network
isolation, race-free TOCTOU behavior, rollback, absence of ambient external
effects, automatic reconnect, automatic authority restoration, or post-start
HostExplicit cancellation. No model, frontend, provider, permission, or
confirmation input amplifies capability authority.

## Workspace / dependency gate

The workspace must contain 13 packages, all version `0.20.0`, edition 2024.
Cargo.lock may change only in the 13 RAH workspace package version entries.
Third-party dependency versions, checksums, dependency edges, additions, and
removals must not drift.

## Validation checklist

Run sequentially and record actual results in the Task 242 preparation plan:

- Codex baseline verification when locally available;
- `cargo fmt --check`;
- `cargo check --workspace`;
- `cargo test -p rah-tools authorized_dispatch -- --nocapture` (historical 15);
- `cargo test -p rah-runtime-codex`;
- `cargo test -p rah-desktop`;
- `cargo test --workspace`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `node --check crates/rah-desktop/frontend/status.js`;
- `node crates/rah-desktop/frontend/status_authority_test.js`;
- `cargo build -p rah-desktop --release`;
- `git diff --check`;
- `cargo metadata --no-deps --format-version 1`;
- Cargo.toml/Cargo.lock workspace-only diff inspection.

Do not run the Windows live HostExplicit gate, Task 229 model attempts, an
AgentRequest, a model prompt, a required-Tool experiment, or another branch
effect.

## Publication boundary

Task 242 creates no `v0.20.0` tag and no GitHub Release. Task 243 alone may use
the immutable Task 242 release-preparation SHA to create one annotated tag,
wait for tag CI, and publish the release. No publication identity may be
invented during preparation.
