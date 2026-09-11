# Task 303 — Ordinary Rename Case-Equivalent Git Collision Correction

## Purpose

Correct the material ordinary `repo.rename-file` ADR 0018 gap identified by
Task 302. This is a focused security correction task, not v0.25 release
preparation.

## Defect

On Windows, `destination_git_absent` currently checks the requested Git path
against HEAD and the index using exact path bytes. Git path lookup is
case-sensitive while the supported Windows filesystem path identity is
case-insensitive. A tracked path can therefore be absent from the worktree,
remain in HEAD/index, and still collide with a differently cased requested
destination without the Git proof rejecting it.

## Scope

- Update ordinary ADR 0018 destination Git collision proof to compare relevant
  HEAD and index paths using the existing supported Windows path-equivalence
  rules.
- Preserve repository, path, source, destination, parent, reparse, nested
  repository, mount/volume, alias, and one-native-attempt constraints.
- Add deterministic coverage for a tracked-but-missing case-equivalent
  destination, including no native attempt and no effect.
- Revalidate the corrected proof and keep known-no-effect and uncertain
  classifications independent and conservative.
- Update only directly affected security/test documentation if needed.

## Out of scope

Do not change the reviewed HostExplicit route, ADR 0021 coordination, ADR 0026
review semantics, frontend/Tauri authority, public ordinary input schema,
workspace version, provider protocol, release tag, or GitHub Release.

## Acceptance evidence

The focused deterministic suite must prove that a tracked case-equivalent Git
collision is rejected before any native rename attempt. Workspace validation,
security documentation consistency, and exact-head CI must pass before a new
milestone audit is attempted.
