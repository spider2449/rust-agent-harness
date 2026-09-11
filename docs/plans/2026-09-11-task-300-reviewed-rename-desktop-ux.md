# Task 300 — RAH v0.25 Reviewed Rename Desktop UX and Tauri Permission Wiring

## Scope

Expose the accepted ADR 0026 reviewed `repo.rename-file` HostExplicit route
through the existing Desktop UI and authorize only its Tauri Prepare command.
This task is frontend/Desktop integration and deterministic evidence only.

## Authority and input boundary

The frontend is presentation and human input only. The eligible Effective
Authority entry renders exactly two repository-relative logical path fields:
`source_path` and `destination_path`. Prepare sends only those two fields to
`host_prepare_repo_rename_file`. The frontend never constructs hashes, byte
lengths, content, Tool data, permission data, filesystem or repository
identity, native paths, Git state, currentness, generations, or authority
objects.

## Review and lifecycle

The backend-derived complete review is shown without truncation or local
filesystem reconstruction. It includes the operation, distinct source and
destination paths, source byte length, SHA-256, format, mode, complete escaped
source content, expected effect, expected Git consequence, and all explicit
non-effects. The existing in-memory reviewed Host dialog owns the opaque
prepared state. Confirm and Cancel use the existing generic commands with
ticket-only requests; no paths or review material are resent, and no cancel
path attempts to undo a rename.

Terminal presentation is status-only and uses backend classification:
`renamed_verified`, `known_no_effect`, `invalid_input`,
`precondition_failed`, and `uncertain`. Raw ToolOutput and private retained
evidence are not rendered or persisted in generic activity. The verified
message describes independent source-absence and destination-match proof and
the unstaged, non-Stage/non-Commit effect. Uncertain results explicitly state
that RAH does not retry, reverse, roll back, or compensate.

## Tauri permission wiring

The Windows Desktop build command manifest includes only the new
`host_prepare_repo_rename_file` command for this task. Its generated narrow
permission is `allow-host-prepare-repo-rename-file`, and the default
capability opts into exactly that permission. Confirm and Cancel retain their
existing permissions. No generic filesystem or wildcard permission is added.

## Privacy and currentness

The form is rendered only when the backend Effective Authority entry reports
HostExplicit eligibility and `repo_rename_file`. Existing authority refresh,
busy-state, stale-review, and safe backend-error behavior remains in charge of
currentness. Review content is intentionally visible only in the reviewed
dialog and does not enter generic persisted activity; tickets and private
evidence remain process-local backend state.

## Deterministic evidence

Frontend static coverage checks the eligible two-field form, exact Prepare DTO,
absence of frontend-derived security fields and direct filesystem APIs,
complete review fields/content/non-effects, ticket-only Confirm/Cancel, safe
error handling, result semantics, activity privacy, currentness gating, and
duplicate-confirmation prevention. A separate permission test checks the
manifest command, generated allow permission, default capability opt-in,
absence of generic filesystem permissions, and unchanged Confirm/Cancel
permission entries.

Validation includes Node syntax checks for modified frontend JavaScript,
focused Desktop tests, workspace formatting/check/test/clippy gates, and
`git diff --check`. Windows live certification, release preparation or
publication, backend authority redesign, eligibility changes, and the next
live reviewed-rename gate are explicit non-goals.
