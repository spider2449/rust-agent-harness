# Task 334 — Remembered Workspace Desktop UX and Privacy Presentation

## Scope

Add the Desktop presentation and closed IPC needed to curate remembered
workspace candidates while keeping remembered persistence descriptive and
separate from process-local repository membership and activation.

The generic catalog remains privacy-safe. Full location hints are returned
only by an explicit human-triggered reveal command. The frontend uses the
existing Tauri dialog plugin for directory selection, does not persist
browser state, and does not perform repository probing or authority decisions.

## Implementation

- Add a narrow `reveal_remembered_workspace_location` command and deterministic
  tests for exact stored hints, bounded failures, and no authority/path-probe
  side effects.
- Add the remembered-workspaces section, add/edit/delete/reorder controls,
  explicit reveal/hide controls, and separate inert admission feedback to the
  existing Desktop shell.
- Add exact generated Tauri command permissions without wildcard permissions.
- Add deterministic frontend static checks for privacy, command routing,
  accessibility, and admission/activation separation.

## Validation and delivery

Run the existing frontend Node checks and JavaScript syntax checks, the
requested `rah-desktop` format/check/test/clippy/diff validation, and the
Desktop release build. Commit and push the validated head to `origin master`,
then verify clean exact-head state and passing exact-head CI.
