# RAH v0.8 Release Evidence

Release status: **RELEASED**

Publication date: 2026-08-25

## Release identity

- Release: `RAH v0.8.0`
- Tag: `v0.8.0`
- Release commit: `0b12d5448dcea89b158e4941e7b741b7539c8894`
- Annotated tag object: `198eccd34a8ae76b9235736c3d1a64173692c351`
- Peeled tag target: `0b12d5448dcea89b158e4941e7b741b7539c8894`
- Release-preparation CI: `32804191964` (`completed / success`)
- Tag CI: `32804873958` (`completed / success`)
- GitHub Release: [RAH v0.8.0](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.8.0)
  (ID `376127345`; published `2026-08-25T03:29:45Z`; draft: false;
  prerelease: false).
- Version: `0.8.0` across all 11 workspace packages (edition 2024).
- ADR 0013: Accepted.
- Certified Codex executable: exactly `codex-cli 0.149.0` with SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Certified isolated configuration: model `gpt-5.4`, reasoning `medium`,
  fingerprint `d967dc569062346bb9dd3084fef0f004842e36044a301d49e936a84b31ad0f7d`.

## Milestone scope and authority

v0.8 contains bounded repository file creation through canonical
`repo.create-file`. ADR 0013 is Accepted and remains distinct from ADR 0012:
`repo.patch` changes an existing tracked pathname, while `repo.create-file`
exclusively creates one previously absent UTF-8 file. The creation capability is
host-bound, uses a private policy and a closed `{path, content}` schema, and
uses `Execute` only as an outer dispatch gate. It grants no arbitrary filesystem
write, overwrite, index/history authority, or public policy escalation path.

## Deterministic evidence

The implementation uses a shared per-repository mutation lease with
`repo.patch`; native creation is descriptor/handle-relative and exclusive.
Windows uses handle-relative `NtCreateFile`, `FILE_CREATE`, parent-handle
identity, and reparse/junction rejection. Unix uses directory FDs, `openat`,
`O_NOFOLLOW`, `O_CREAT | O_EXCL`, and intended `0o600` mode subject to umask.

Deterministic tests cover closed schema/Execute gating, Git HEAD/index/
intent-to-add/conflict/ignored/submodule/sparse preconditions, raw-index-byte
invariants, no overwrite, race and parent-replacement protection, bounded
partial-write classification, uncertain outcomes, no deletion, and no replay.
Trusted-profile composition remains version 1, uses symbolic host resources,
is non-mutating for static/effective composition, publishes a fresh redacted
registry, and does not accept authority from model or provider metadata. The
Generic Tool Bridge remains generic: private aliases, Execute enforcement,
dedupe, no retry after known/uncertain writes, and redacted output translation.

## Windows certified live evidence

Task 087 established the certified native `codex-cli 0.149.0` baseline with
the executable SHA-256 and isolated configuration recorded above. Binary version
alone is insufficient: the executable, hash, isolated configuration, model,
reasoning, fingerprint, and RAH source commit are all required release evidence.

Task 089 completed three fresh pre-commit and three fresh post-commit release
runs through the actual Trusted Profile -> fresh ToolRegistry -> Generic Tool
Bridge chain. Every run emitted `RAH_CREATE_FILE_LIVE_OK` after
the host verifies exactly one create request/start/finish/native operation,
completed observers, exact untracked target, unchanged raw index/HEAD/refs/
sentinel, `Completed`, cleanup, and diagnostic-only model final prose.

The fixture disables alternate Codex mutation paths, including shell,
unrestricted file write, arbitrary process, Codex-owned MCP, web/network,
apps, and plugins.

## Ubuntu deterministic evidence

Windows provides native/live release evidence and Windows-specific deterministic
path hardening. Exact release-commit Ubuntu CI run `32804191964` completed
successfully. Ubuntu provides deterministic CI and Unix native-test evidence
only; this milestone makes no Unix live Codex claim.

## Known limitations

`repo.create-file` creates one UTF-8 file per call at an existing parent; it
does not overwrite, delete, rename, create directories, create binary files,
stage, commit, alter history, or provide a multi-file transaction. Partial
writes can remain after a possible effect. There is no rollback, automatic
replay, cross-process lock, or OS-sandbox guarantee.

## Completed release checklist

- [x] Deterministic workspace gates and metadata validation passed.
- [x] Certified `codex-cli 0.149.0` executable, hash, and isolated
  configuration were verified.
- [x] Three fresh pre-commit and three fresh post-commit Windows live runs
  passed through the Trusted Profile and Generic Tool Bridge.
- [x] Exact release-commit Ubuntu deterministic CI completed successfully.
- [x] Immutable annotated tag identity and peeled release commit were verified.
- [x] The non-draft, non-prerelease GitHub Release was published.

## Historical record

This document is the historical v0.8.0 release gate. It is no longer a pending
release-preparation checklist. The immutable `v0.8.0` annotated tag remains on
release commit `0b12d5448dcea89b158e4941e7b741b7539c8894`; post-release
documentation cleanup advances `master` without moving or recreating that tag.
