# RAH v0.9 Release Gate

Release status: **RELEASED**

## Release identity

- Release: `RAH v0.9.0`
- Tag: `v0.9.0`
- Tag object: `fbb30c3787911bdb935417bf51d9c0c5f2bdf381`
- Release commit / peeled target: `d971790fd1de7df782a99d2274278a14f1f0066f`
- Version: `0.9.0` across all 11 workspace packages (edition 2024).
- ADR 0014: Accepted.
- Certified Codex baseline: exactly `codex-cli 0.149.0`.
- Windows live marker: `RAH_REPO_EDIT_FILES_LIVE_OK`.
- Release-preparation CI: `32823991579` — success.
- Tag CI: `32824354008` — success.
- GitHub Release: published (numeric ID `376240486`).
- Draft / prerelease: `false` / `false`.

## Milestone evidence

Task 096 completed the certified Windows live path:

```text
TrustedStaticProfile
 -> effective composition
 -> fresh ToolRegistry
 -> Generic Tool Bridge
 -> repo.edit-files
 -> two committed_verified target edits
 -> Completed
```

The host verified one attempt per target, deterministic lexical commit order,
unchanged raw index/HEAD/refs, and the structural marker after cleanup.

## Known limitations

`repo.edit-files` is not a cross-file transaction and has no rollback, retry,
or replay. It permits only one through four existing clean HEAD-tracked UTF-8
files with exact original-snapshot preconditions. It grants no generic
filesystem write; no create, delete, rename, staging, commit, history, ref, or
network Git authority. Unix live Codex validation is not claimed.

## Release checklist

### Completed before tag

- [x] Deterministic release gates.
- [x] Windows certified live gate with exactly `codex-cli 0.149.0`.
- [x] ADR 0014 Accepted.
- [x] Trusted Profile v1 and Generic Tool Bridge integration.
- [x] Version bump to `0.9.0` across 11 packages.

### Completed publication

- [x] Annotated `v0.9.0` tag.
- [x] Tag CI success.
- [x] GitHub Release publication.
- [x] Immutable tag and peeled-commit verification.

## Historical record

The `v0.9.0` tag is immutable; later `master` commits do not move this release.
This file is historical release evidence rather than a pending gate.
