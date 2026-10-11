# RAH v0.19.0 Release Gate

**RELEASED — HISTORICAL RECORD**

## Release identity

Release: RAH v0.19.0

Milestone: Bounded Local Branch Creation at Captured Attached HEAD

Milestone audit: `757340061084b8fd6f591b9d985ada2c59b9113c`

Task 230 exact-head CI: `34038812072` PASS

Release commit: `88463787d14e32a3f6692385161b59474bff1dea`

Annotated tag: `v0.19.0`

Tag object: `24fff2dbae77d6fbd281bf9f4ac34b855c116e3a`

Peeled target: `88463787d14e32a3f6692385161b59474bff1dea`

Task 231 exact-head CI: `34040345951` PASS

Tag-triggered CI: `34071020347` PASS

GitHub Release numeric ID: `383770772`

GitHub Release node ID: `RE_kwDOT-U4RM4W3-CU`

GitHub Release URL:
https://github.com/spider2449/rust-agent-harness/releases/tag/v0.19.0

draft: `false`

prerelease: `false`

publication date: `2026-09-07`

Verdict: **RELEASED WITH DOCUMENTED LIMITATION**

The immutable release source is the release commit above. Later documentation
cleanup commits on `master` are not part of the v0.19.0 source identity.

## Product and authority contract

RAH v0.19.0 adds the accepted ADR 0020 bounded local branch-creation
capability, `repo.create-branch`. The host owns a private
`RepositoryBranchCreationPolicy` behind an opaque
`RepositoryBranchCreationAuthority`. Desktop composes it only for the
selected repository when valid host branch authority is present; Execute is
the outer permission gate and is not authority by itself.

The model supplies only `{"name":"<validated-logical-branch-name>"}`. The
host validates a closed bounded ASCII name, captures the attached committed
`HEAD` OID, constructs only `refs/heads/<name>`, and performs one fixed native
Git create-only CAS:

```text
git update-ref --create-reflog -m "RAH create local branch"
  refs/heads/<name> <captured-head-oid> <zero-old-oid>
```

The zero-old OID requires expected absence. One authorized call creates at
most one absent ordinary local branch and may create only its Git-owned fixed
reflog. It does not switch or checkout, create-and-switch, overwrite, force,
delete, rename, set tracking/upstream, mutate the index/worktree, alter
commit/history, access tags/remotes, or provide generic/remote Git or ref
authority. Trusted Profile/provider metadata and frontend presentation cannot
manufacture branch authority.

Desktop's sanitized Effective Authority classification is:

- public Tool: `repo.create-branch`;
- effect class: `repository_mutation`;
- authority category: `repository_local_branch_creation`;
- permission: `execute`;
- `repositoryBound: true`;
- source: `repository_host` / `desktop_repository`;
- frontend label: `Local branch creation`.

The frontend presents this backend classification only; it does not authorize
or infer authority from the Tool name. Execute alone, startup, or Trusted
Profile/provider metadata cannot make the Tool available.

The capability admits only an ordinary non-bare repository with a real `.git`
directory, conventional files-backed refs, and an existing attached committed
`HEAD`. Dirty, staged, and mixed ordinary states are allowed. Detached or
unborn HEAD, bare repositories, `.git` indirection, linked worktrees,
unsupported ref backends, and required-rejected special Git-operation states
remain outside scope. Repository and executable identity, hooks, configuration,
environment, and output bounds remain host-controlled; race-free external
TOCTOU exclusion is not claimed.

## Evidence chronology

- Task 222: selected the bounded local branch milestone scope.
- Task 223: researched authority, admission, name, collision, CAS,
  confinement, and uncertainty requirements.
- Task 224: accepted ADR 0020.
- Tasks 225/225B: added the private policy and observation hardening.
- Task 226: added the public Tool and authority surface.
- Task 227: completed release hardening and the deterministic branch gate.
- Task 228: integrated Desktop composition and corrected currentness/result
  handling.
- Task 229: recorded two model-selected zero-dispatch attempts — INCONCLUSIVE.
- Task 229B: selected Option B, independent host-driven effect certification.
- Task 229C: certified the Windows host-driven real effect — PASS.
- Task 230: recorded Verdict B — milestone complete with documented limitation.

## Certified live evidence

### MODEL-SELECTED: NOT CERTIFIED

The certified Codex baseline was `0.149.0`, SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, with
model `gpt-5.6-terra`. Task 229 recorded two bounded attempts, each with:

```text
ToolRequested = 0
ToolStarted   = 0
ToolFinished  = 0
```

No target branch effect occurred. These were not branch execution failures;
zero model requests do not prove mutation failure.

### HOST-DRIVEN WINDOWS EFFECT: CERTIFIED

Task 229C separately certified the real production path:

```text
DesktopRepository
 -> RepositoryBranchCreationAuthority
 -> RepositoryBranchCreationTool
 -> Desktop ToolRegistry
 -> exactly one host-driven registry dispatch
 -> real native Git branch effect
```

The fresh-fixture PASS asserted branch creation at the captured committed
`HEAD`, fixed reflog, unchanged symbolic/current `HEAD`, index, staged and
unstaged state, tracking, reviewed authorization, generations, connection,
conversation namespace, and provider state, with no replay or rollback. The
fresh successful branch name and OID were asserted internally but not printed
on the successful output path. This is a nonblocking evidence-capture
limitation; the effectful live gate is not rerun.

Windows host-driven Desktop repo.create-branch authority/effect path is certified. Model-selected repo.create-branch dispatch was not observed in two bounded Codex live attempts and is not certified.

## Preserved limitations

- Model-selected `repo.create-branch` dispatch is not certified.
- The fresh successful branch name/OID echo is missing; values are not
  invented and the effectful test is not rerun.
- Linux live branch certification is not established.
- Task 207 remains INCONCLUSIVE / externally blocked for model-selected MCP /
  Process Plugin Tool execution; v0.19 does not fix or reclassify it.
- Branch switching, checkout, generic Git/ref/history, remote/network Git,
  branch delete/rename/force, and tracking are not included.
- Trusted Profile/provider metadata cannot grant branch authority.
- Uncertain effects are not replayed, retried, rolled back, or compensated.
- Process supervision is not OS sandboxing; network isolation is not claimed.
- External TOCTOU races are not eliminated or claimed race-free.

## Workspace and dependency gate

The prepared workspace must contain 13 packages, all at version `0.19.0`, with
edition 2024. Cargo.lock changes must be limited to workspace-version
transitions caused by this bump. No third-party dependency version, checksum,
addition, or removal may change. Release preparation introduces no production
authority change.

## Validation and publication checklist

The following local checks are recorded from the Task 231 run:

- [x] certified baseline verification — PASS (`codex-cli 0.149.0`)
- [x] `cargo fmt --check` — PASS
- [x] `cargo check --workspace` — PASS
- [x] focused `rah-tools` branch gate — PASS, 29 passed
- [x] `cargo test -p rah-desktop` — PASS, 180 passed, 4 ignored
- [x] `cargo test --workspace` — PASS, all suites; live-only tests remained
  ignored
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS
- [x] frontend syntax check — PASS
- [x] frontend authority static test — PASS
- [x] Desktop release build — PASS
- [x] `git diff --check` — PASS
- [x] metadata audit — PASS, 13 packages, all `0.19.0`, edition 2024
- [x] documentation consistency and limitation audits — PASS
- [x] exact-head pre-commit scope audit — PASS; only the eight expected files
  are staged, with no `crates`, `.github`, or `docs/adr` diff

- [x] preparation exact-head CI PASS (`34040345951`)
- [x] annotated v0.19.0 tag created
- [x] tag object recorded (`24fff2dbae77d6fbd281bf9f4ac34b855c116e3a`)
- [x] tag pushed
- [x] GitHub Release published (`383770772`)
- [x] draft=false
- [x] prerelease=false
- [x] tag-triggered CI PASS (`34071020347`)
- [x] immutable release commit verified
- [ ] post-release-cleanup exact-head CI (Task 233; pending after push)
