# RAH v0.9 Release Gate

Release status: **RELEASE READY / PENDING TAG**

## Release identity

- Release: `RAH v0.9.0`
- Tag: `v0.9.0` (pending)
- Release commit: pending Task 097 commit SHA; Task 098 will use the immutable
  release-preparation commit.
- Version: `0.9.0` across all 11 workspace packages (edition 2024).
- ADR 0014: Accepted.
- Certified Codex baseline: exactly `codex-cli 0.149.0`.
- Windows live marker: `RAH_REPO_EDIT_FILES_LIVE_OK`.
- Ubuntu release-preparation CI: pending this Task 097 exact-head run.

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

### Pending Task 098

- [ ] Annotated `v0.9.0` tag.
- [ ] Tag CI success.
- [ ] GitHub Release publication.
- [ ] Immutable tag and peeled-commit verification.
