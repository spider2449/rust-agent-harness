# RAH v0.8 Release Gate

Status: Milestone audit complete; release preparation has not started.
Date: 2026-08-25
Audit HEAD: `4347bbf990e316a16e483513e918bfc537bfc61d`

## Milestone scope

v0.8 contains only bounded repository file creation through canonical
`repo.create-file`. Multi-file editing, richer patch syntax, Git commit/history
authority, session persistence, network MCP, `PluginManager`, and dynamic
profile reload remain out of scope.

## Authority and ADRs

ADR 0013 is Accepted and remains distinct from ADR 0012: `repo.patch` changes
an existing tracked pathname, while `repo.create-file` exclusively creates one
previously absent UTF-8 file. The creation capability is host-bound and uses a
private policy, a closed `{path, content}` schema, and `Execute` only as an
outer dispatch gate. It grants no arbitrary filesystem write, overwrite,
rollback, transactional creation, index/history authority, or public policy
escalation path.

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

## Certified live evidence

Task 087 certified exactly native `codex-cli 0.149.0` with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, isolated
`CODEX_HOME`, model `gpt-5.4`, reasoning `medium`, and fingerprint
`d967dc569062346bb9dd3084fef0f004842e36044a301d49e936a84b31ad0f7d`.
Binary version alone is insufficient: the executable, hash, isolated
configuration, model, reasoning, fingerprint, and RAH source commit are all
required release evidence.

Three fresh pre-commit and three fresh post-commit runs exercised the actual
Trusted Profile -> fresh ToolRegistry -> Generic Tool Bridge chain. Each run
called `repo.create-file`, `repo.file-info`, and `repo.status` exactly once.
The host, not model prose, verified `src/live_marker.rs` as a regular,
non-reparse, untracked 81-byte file with SHA-256
`8cd485928d7faeded7a85802d96e91220ab27feffa1e0761eeab6c949996345b`, preserved
raw index/HEAD/refs/sentinel state, observed `Completed`, cleaned up, and then
emitted `RAH_CREATE_FILE_LIVE_OK`.

The fixture disables alternate Codex mutation paths, including shell,
unrestricted file write, arbitrary process, Codex-owned MCP, web/network,
apps, and plugins. Final assistant prose is diagnostic-only.

## Platform evidence and limitations

Windows provides the native/live release evidence and Windows-specific
deterministic path hardening. Ubuntu CI provides deterministic CI and Unix
native-test evidence only; this milestone makes no Unix live Codex claim.

Partial writes can remain after a possible effect. There is no rollback,
transaction, automatic replay, cross-process lock, or OS-sandbox guarantee.

## Release blockers

None found by the Task 088 audit. The factual documentation defects found at
the audit head were corrected before this gate was recorded.

## Verdict

**RELEASE READY** — the RAH v0.8 milestone is ready for release preparation.
This document is a pre-release gate and does not mark v0.8 as released.
