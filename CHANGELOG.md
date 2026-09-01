# Changelog

## v0.12.0 — 2026-09-01

Released as `RAH v0.12.0` for the audited Desktop repository-authoring
milestone. The immutable release commit is
`d1c1cd470fd337f141abb9675fb4642ccd2e00b0`; annotated tag `v0.12.0` has
object ID `4d002a8bc67b1877e692bb0aafd764fc5eb47b65` and peels to that commit.
Task 153 candidate CI run `33479033004` and Task 154 tag CI run `33479751261`
passed. The GitHub Release was published on `2026-09-01T06:58:38Z`.

### Added

- Windows Desktop end-to-end bounded repository workflow: model bounded
  repository authoring, human Stage / Unstage, host-observed staged review,
  human reviewed-snapshot authorization, message-only `repo.commit`, verified
  Git commit result, and Desktop repository refresh.

### Verified

- The existing authority boundaries are productized through Desktop; v0.12
  introduces no new authority. Model request is not authorization; Execute
  permission is not commit authorization; human Stage / Unstage are host
  actions; human Authorize is the reviewed-snapshot authorization event; and
  the frontend does not own authorization.
- `RepositoryCommitReview` remains opaque and Rust-only. `repo.commit` remains
  message-only, does not auto-stage, and uncertain external effects are not
  replayed.
- Windows live validation reused the Task 151 certified `codex-cli 0.149.0`
  bundle with closed manifest schema v2. The demonstrated complete pair is
  `codex.exe` and `codex-code-mode-host.exe`, each with a closed identity and
  SHA-256; the Task 151 hardening closes only the Desktop verifier gap that
  could accept a directory missing the code-mode host.
- The bounded repository-safe authoring path was live-proven. At
  `D:\\rah-task151-clean`, exactly one independently verified Git commit effect
  was observed: `90683f5eaab129a75e815879e69586ff75de5e86`, with no second
  commit or replay. This fixture commit is not the RAH v0.12 release commit.

### Security and limitations

- No generic Git, shell/process, or `fs.write` authority is introduced. There
  is no branch/ref, network, credential, or rollback authority.
- The exact live edit Tool label and exact live `repo.commit` activity-event
  counts were not durably retained. They remain documented non-blocking
  observability gaps; no unsupported lifecycle counts are asserted.
- Windows is live-certified. Unix/macOS live validation is not claimed. Task
  120 remains **DEFERRED / NOT VALIDATED** and transport confinement remains
  **NOT CLAIMED**.

## v0.11.0 — 2026-08-30

Released as `RAH v0.11.0`. The immutable annotated tag `v0.11.0` has object
ID `3fd37807f382c2c0c61328e72d7542984db05983` and peels to release commit
`44a2ee3c6580b862fd0a71b9e773984de757dc15`. Tag CI run `33300410414`
passed, and the GitHub Release was published.

### Added

- `repo.commit`, a bounded host-reviewed repository commit capability that
  creates one ordinary commit from one exact reviewed staged snapshot in the
  exact trusted-profile-selected repository.
- Trusted Profile composition for the exact repository, exact native Git
  executable, explicit trusted host identity, Execute outer permission, and
  separate host-only per-operation authorization.

### Verified

- Deterministic commit-policy hardening, Trusted Profile composition, and
  Generic Tool Bridge verification.
- Windows certified live Codex validation at exactly `codex-cli 0.149.0`; the
  complete same-version official code-mode host was required for the certified
  dynamic-tool path.
- The disposable live fixture completed lifecycle `1 / 1 / 1` with
  `committed_verified` at `13c200c5c772b3e4a0eceb0a2364981c849313e0`.
  This is fixture evidence, not a RAH repository release commit. There was no
  automatic staging, retry, replay, approval, or synthetic tool call.

### Security and limitations

- `repo.commit` is not generic Git authority. Execute alone is insufficient:
  every commit requires fresh host-reviewed authorization; the model controls
  only the message.
- No automatic staging, branch creation/switching, arbitrary ref mutation,
  detached/unborn commit, amend, merge, rebase, cherry-pick, reset, clean,
  stash, tag, remote/network Git, credential Git, linked worktree, or
  submodule/gitlink commit is supported.
- Uncertain effects are never retried or replayed, and no rollback guarantee is
  made. Windows is live-certified; Ubuntu is deterministic evidence only.
- Task 120 remote llama generation remains **DEFERRED / NOT VALIDATED**.
  Transport confinement remains **NOT CLAIMED**.

## v0.10.0 — 2026-08-29

Released as `RAH v0.10.0`. The immutable annotated tag `v0.10.0` has object
ID `d340120e5b316265d6a4cd83bdf08eb73d712d1a` and peels to release commit
`9f4947ce4e37e9ce5b1e49330ab5327c1bd61ffa`. Tag CI run `33248727210`
passed, and the GitHub Release was published.

### Added

- Desktop certified Codex baseline discovery and selection for exactly
  `codex-cli 0.149.0`, plus closed native Git executable discovery.
- One bounded host-selected llama.cpp provider endpoint under ADR 0015, inactive
  Desktop model-preference persistence, exact selected-repository observation,
  verified repository runtime-CWD binding, and launch-CWD/`AGENTS.md` isolation.
- Repository-scoped conversation persistence, explicit bounded Resume/replay,
  SQLite transcript storage, transactional V3-to-SQLite migration, A/B
  transcript isolation, and fail-closed SQLite corruption handling.

### Limitations

- Task 120 remote llama.cpp generation proof is **DEFERRED / NOT VALIDATED**.
  Transport confinement is **NOT CLAIMED**; ADR 0015 does not promise redirect,
  proxy, DNS, peer-identity, or effective-destination confinement.
- No llama.cpp process management or provider/model installation; no generic
  network Tool, network MCP/Streamable HTTP, generic shell/process authority,
  model-selected executable/cwd/endpoint, automatic authority restoration, Git
  commit/ref/history authority, or generic repository delete/rename authority.
- Repository move/rename intentionally changes the conversation-persistence
  namespace. SQLite is private Desktop storage, not generic SQL authority, and
  uncertain external effects have no rollback guarantee.

## v0.9.0 — 2026-08-25

Released as `RAH v0.9.0` on 2026-08-25. The immutable annotated tag `v0.9.0`
has object ID `fbb30c3787911bdb935417bf51d9c0c5f2bdf381` and peels to release
commit `d971790fd1de7df782a99d2274278a14f1f0066f`. Tag CI run `32824354008`
completed successfully, and the GitHub Release was published.

### Added

- `repo.edit-files` bounded multi-file repository edit authority for up to four
  existing, clean, tracked UTF-8 files, with exact original-snapshot
  replacements and deterministic host-owned commit order.
- Verified partial-effect and uncertain-outcome semantics, Trusted Profile v1
  composition, and Generic Tool Bridge integration.

### Verified

- Windows certified Codex live validation using exactly `codex-cli 0.149.0`
  emitted the structural marker `RAH_REPO_EDIT_FILES_LIVE_OK`.
- ADR 0014 is Accepted.

### Security and limitations

- `repo.edit-files` is not a cross-file transaction and provides no rollback
  or replay.
- It grants no generic filesystem write; it cannot create, delete, or rename
  files, and grants no staging, commit, history, ref, or network Git authority.
- Unix live Codex validation is not claimed.

## v0.8.0 — 2026-08-25

Released as `RAH v0.8.0` on 2026-08-25. The immutable annotated tag `v0.8.0`
has object ID `198eccd34a8ae76b9235736c3d1a64173692c351` and peels to release
commit `0b12d5448dcea89b158e4941e7b741b7539c8894`.

### Added

- Bounded repository file creation through `repo.create-file`: one
  host-authorized UTF-8 file at an existing parent directory per call.
- Native exclusive creation with no overwrite, composed through the Trusted
  Profile and the Generic Tool Bridge while retaining host-bound repository
  authority.

### Verified

- Deterministic Windows and Linux coverage, plus certified Codex live
  validation using exactly `codex-cli 0.149.0`.
- The release-preparation CI run `32804191964` and tag CI run `32804873958`
  completed successfully. The GitHub Release was published at
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.8.0>.

### Limitations

- No overwrite, delete, rename, directory creation, binary creation, staging,
  commit/history authority, multi-file transaction, rollback, or replay.
- `repo.create-file` creates one file per call and requires its parent to
  already exist.

## v0.7.0 — 2026-08-24

Released as `RAH v0.7.0` on 2026-08-24. The immutable annotated tag `v0.7.0`
has object ID `b4df68290053f7dd8f6a2b45671fd7cdab8d128f` and peels to release
commit `9521fa4e5f5c184eabd0061eb71854422752b8f1`.

### Added

- `repo.patch` retains its legacy single-replacement request and additionally
  accepts `replacements[]` with one through sixteen exact replacements in one
  existing, HEAD-tracked, regular UTF-8 worktree file.
- Every replacement is resolved against the same original snapshot. Duplicate,
  overlapping, absent, and ambiguous matches are refused; accepted
  non-overlapping replacements are applied deterministically in one final
  single-file replacement.
- Full-file SHA-256 and byte-length preconditions remain mandatory. The
  operation does not automatically stage changes.

### Verified

- Deterministic Generic Tool Bridge validation, Windows native Codex live
  multi-replacement validation, and repository-observer verification cover the
  milestone. Reproducible certified baseline tooling uses isolated configuration
  and host-attested structural markers; the Codex platform-alignment audit
  remains part of the release evidence. Tag CI run `32706469848` completed
  successfully.
- The certified live runtime is exactly `codex-cli 0.149.0`. This release does
  not claim Unix live Codex validation.

### Security and limitations

- `repo.patch` is not arbitrary filesystem write authority. It does not create,
  delete, or rename files; provide a multi-file transaction or rollback; or
  grant Git commit, history, ref, or network authority.
- It does not grant generic shell or process authority. Model requests and
  Codex approvals remain non-authoritative; host policy and `ToolRegistry`
  checks remain required.

## v0.6.0 — 2026-08-24

Released repository-aware read-only workflow inspection milestone. The immutable
annotated tag `v0.6.0` peels to
`6326c18937bbcfd1e515001692a2c88c6884d552`. The GitHub Release, titled
`RAH v0.6.0`, was published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.6.0>.

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
- Release-preparation CI run `32685119256` and tag CI run `32685443380`
  completed successfully. This release does not claim Unix live Codex
  validation, transactional snapshot consistency, or zero incidental filesystem
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
