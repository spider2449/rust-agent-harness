# Changelog

## v0.6.0 — 2026-08-24 (release preparation)

Release preparation for the repository-aware read-only workflow inspection
milestone. Tagging and GitHub Release publication are intentionally deferred
until the release-preparation commit has passed its required CI.

### Added

- A repository-aware read-only observer toolkit: `repo.file-info`,
  `repo.status`, `repo.diff`, and `repo.diff-staged`.
- Trusted-profile composition for the four fixed host observer capabilities,
  deterministic Generic Tool Bridge verification, and Windows live Codex
  verification using exactly `codex-cli 0.149.0`.
- Ubuntu deterministic and cross-platform coverage for repository-observer
  behavior.

### Security

- No new mutation authority. Observers are fixed-command host capabilities;
  `PermissionLevel::Execute` is only their outer host-process gate.
- The observers do not provide generic Git execution or arbitrary executable,
  argv, cwd, or environment selection. They disable external diff and textconv
  behavior and make no intentional repository mutation.
- The existing guarded `repo.patch` worktree mutation capability remains
  separately governed by ADR 0012. ADR 0010 remains repository-index mutation
  only, and ADR 0011 remains trusted-profile authority composition.

### Verified

- Task 064 deterministically verified all four observers through the Generic
  Tool Bridge. Task 065 ran three fresh Windows live fixtures with exactly
  `codex-cli 0.149.0`; each observer was invoked once and the repository was
  unchanged.
- Task 066 recorded the `RELEASE READY` milestone decision and Ubuntu CI run
  `32684390117` as completed successfully. This release does not claim Unix
  live Codex validation, snapshot consistency, or zero incidental filesystem
  writes.

## v0.5.1 — 2026-08-22

Released and published as `v0.5.1`, tagged at
`0ea648d84d6f48720c33e8b1bb07e1c24101c870`. This is the portability-only
recovery release for the published v0.5.0 repository-mutation milestone; it
adds no authority, behavior, public API, dependency, or feature expansion.

### Fixed

- Corrected a Linux/Ubuntu clippy portability defect by importing
  `std::fs::File` only for the Windows-native repository identity path that
  uses it. The Ubuntu `unused import: File` failure is removed without changing
  `repo.patch` replacement, policy, or test behavior.

### Verified

- The minimal recovery commit passed the required GitHub Ubuntu CI job,
  including formatting, workspace check, workspace tests, and clippy.
- The release-preparation CI run `32574019502` and tag CI run `32574129999`
  both completed successfully. v0.5.1 is the corrected, fully required-CI
  verified v0.5.x baseline.
- The Windows live `repo.patch` release gate remains valid using exactly
  `codex-cli 0.149.0`; this release makes no Unix live Codex validation claim.

## v0.5.0 — 2026-08-22

Published feature release, tagged at
`b1f0fb4a903a59e0b5c23ca107d7508ebcbd8786`. It contains the complete v0.5
`repo.patch` feature milestone and passed Windows release validation. Its
required Ubuntu CI later failed only because `std::fs::File` was imported
unconditionally while used only by the Windows native-identity path. This was a
lint-only portability defect, not a `repo.patch` authority or runtime-semantics
defect. The public release and immutable tag were preserved unchanged; v0.5.1
supersedes v0.5.0 operationally as the fully verified v0.5.x baseline.

### Added

- `repo.patch`, a repository-aware capability that conditionally replaces one
  exact literal text occurrence in one bounded existing, HEAD-tracked, unstaged
  UTF-8 worktree file.
- ADR 0012, accepted: a separate private, host-owned
  `RepositoryWorktreeMutationPolicy` for worktree-content mutation. The existing
  `PermissionLevel::Execute` is only an outer runtime gate, not the authority.
- Whole-file SHA-256 and byte-length preconditions; exact single-match
  replacement; bounded request, source-file, and postimage sizes; and strict
  UTF-8 handling that preserves a leading BOM and CRLF/LF bytes exactly.
- Repository, path, link/reparse-point, and hard-link protections; a
  same-directory exclusive temporary complete postimage; one-attempt/no-replay
  behavior; and known-failure versus uncertain-effect classification.
- Trusted-profile composition of `repo.patch`, Generic Tool Bridge verification,
  and a Windows live validation using exactly `codex-cli 0.149.0`. Restricted
  Codex-owned filesystem, shell, process, MCP, and network-tool capabilities
  remain disabled in that path.

### Security

- Worktree content mutation, index mutation, and Git history/ref mutation are
  separately authorized state planes. `repo.patch` does not grant generic
  filesystem write, generic shell/process, Git command, or network authority.
- The policy accepts one request and one native replacement attempt only.
  Successful results require post-observation; failures are reported as known
  only when the preimage is proven intact. Uncertain outcomes are never replayed.

### Verified

- Deterministic repository-patch, trusted-profile composition, and Generic Tool
  Bridge coverage; the opt-in live gate observed the one-request/one-attempt
  path, preserved index/HEAD/refs and unrelated content, and cleaned its
  temporary repository and app-server child.
- Windows is the verified v0.5 release baseline. This release makes no Unix
  live-validation claim.

### Limitations

- No file creation, deletion, rename/move, binary edits, multi-file
  transactions, staged or untracked target mutation, or `restore-worktree`.
- No Git history/ref mutation, network Git, automatic rollback, complete TOCTOU
  elimination, or Unix live-validation claim.

## v0.4.0 — 2026-08-22

Released 2026-08-22. Tag `v0.4.0` targets release commit `ebd6358`; CI passed
and the GitHub Release was published.

### Added

- Trusted static capability profiles with strict versioned parsing, hardened
  explicit source loading, symbolic host resources, built-in composition, and
  redacted static/effective inventories.
- `rah profile validate` for non-spawning static validation and `rah profile
  validate-effective` for explicit effective provider composition.
- Trusted-profile composition for hardened local stdio MCP and Process Plugin
  providers, including exact expected tool/schema admission and explicit host
  permission mapping.
- ADR 0011, the trusted capability profile authority boundary.

### Changed

- Effective composition constructs a fresh `ToolRegistry`, preserves declared
  permissions, fails closed on duplicate registration, and retains provider
  lifecycle ownership. Staged providers are cleaned up after later failure.
- The optional Codex adapter baseline is exactly `codex-cli 0.149.0`.

### Security

- Profiles configure existing host authority only; model requests and provider
  metadata remain non-authoritative.
- MCP and Process Plugin providers use native executable validation/revalidation,
  isolated cwd, minimized environment, bounded stdio/lifecycle resources, and
  atomic admission. These controls are not OS sandboxing or network isolation.

### Verified

- Deterministic mixed built-in + MCP + Process Plugin composition, permission
  preservation, redacted inventory, duplicate fail-closed behavior, and staged
  provider cleanup.
- Opt-in trusted-profile Generic Codex Tool Bridge validation using exactly
  `codex-cli 0.149.0`: one `plugin.test.echo` execution, Codex continuation,
  and child/app-server cleanup.

### Deferred

- Profile discovery, reload, editing, or mutation; generic provider and
  subprocess schemas; MCP Streamable HTTP/network MCP; PluginManager;
  provider/plugin installation or download; automatic restart; and hot reload.
- Generic shell/process authority, model-selected executable/argv/cwd/env,
  destructive worktree authority, Git commit/ref/history mutation, network or
  credential-bearing Git, OS sandboxing, network isolation, and rollback.

## v0.3.0 — 2026-08-22

Git tag `v0.3.0` was created at release commit `1968326`.

### Verified

- Generic Tool Bridge, `fs.read`, the MCP adapter, and the process-plugin
  adapter remain available through RAH-owned neutral tool boundaries.
- Hardened `HostExecutionPolicy` is verified through deterministic and opt-in
  live fixture validation.
- Host-owned Execute capabilities are `host.cargo.version`, `host.git.status`,
  `host.git.stage`, and `host.git.unstage`.
- `RepositoryMutationPolicy` is verified through deterministic and opt-in live
  repository-mutation fixture validation; `host.git.stage` and
  `host.git.unstage` have deterministic and opt-in live validation.
- The optional Codex adapter baseline is exactly `codex-cli 0.149.0`.

### Capability classification

`process.test.echo` is the hardened Execute validation fixture, and the
repository-mutation fixture validates mutation policy behavior. Neither is a
production/public host capability. In particular, v0.3.0 does not include
`host.fixture.echo`.

### Deferred

- arbitrary `shell.exec` and `process.exec`;
- model-selected executable, argv, cwd, or environment;
- worktree restore and arbitrary file mutation;
- Git commit, refs/history mutation, reset, clean, checkout, switch, stash,
  merge, rebase, push, pull, fetch, network Git, and credential-bearing Git
  execution.

Destructive worktree authority is deferred beyond v0.3 and requires ADR 0011.

### Security notes

Process supervision is not OS sandboxing. RAH makes no network-isolation or
rollback guarantee. Timeout or cancellation can leave uncertain mutation
effects, and uncertain mutations are never automatically replayed. On Windows,
Job Object assignment remains post-spawn; external OS processes can race
repository mutation, and Git configuration may still influence Git semantics.
