# RAH v0.8 Release Gate

Status: Release preparation in progress; not tagged or published.
Date: 2026-08-25
Audit HEAD: `da4667fba3f5e8097a160ed7ec8926ca2b0e1d4f`
Release candidate commit: Task 089 commit, pending.

## Release identity

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

## Certified live evidence requirement

Task 087 established the certified native `codex-cli 0.149.0` baseline with
the executable SHA-256 and isolated configuration recorded above. Binary version
alone is insufficient: the executable, hash, isolated configuration, model,
reasoning, fingerprint, and RAH source commit are all required release evidence.

Task 089 must run three fresh pre-commit and three fresh post-commit release
candidate runs through the actual Trusted Profile -> fresh ToolRegistry ->
Generic Tool Bridge chain. Every run must emit `RAH_CREATE_FILE_LIVE_OK` after
the host verifies exactly one create request/start/finish/native operation,
completed observers, exact untracked target, unchanged raw index/HEAD/refs/
sentinel, `Completed`, cleanup, and diagnostic-only model final prose.

The fixture disables alternate Codex mutation paths, including shell,
unrestricted file write, arbitrary process, Codex-owned MCP, web/network,
apps, and plugins.

## Deterministic and platform evidence

Windows provides native/live release evidence and Windows-specific deterministic
path hardening. Task 088 exact-head Ubuntu CI run `32803452752` completed
successfully; Task 089 requires a new successful exact-head Ubuntu CI run for
its release-preparation commit. Ubuntu provides deterministic CI and Unix
native-test evidence only; this milestone makes no Unix live Codex claim.

## Known limitations

`repo.create-file` creates one UTF-8 file per call at an existing parent; it
does not overwrite, delete, rename, create directories, create binary files,
stage, commit, alter history, or provide a multi-file transaction. Partial
writes can remain after a possible effect. There is no rollback, automatic
replay, cross-process lock, or OS-sandbox guarantee.

## Release-preparation blockers

No milestone blocker was found by the Task 088 audit. Task 089 remains blocked
until its deterministic gates, certified baseline/configuration checks, three
fresh pre-commit live runs, committed exact-head Ubuntu CI, and three fresh
post-commit live runs all pass.

## Verdict

**RELEASE PREPARATION IN PROGRESS** — this document does not mark `v0.8.0` as
tagged, released, or published.
