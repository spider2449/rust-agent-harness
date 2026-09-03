# Task 175A — Generic live Tool evidence completeness hardening

## Scope and historical truth

Task 175 successfully executed `repo.rename-file` on Windows, but its formal
evidence closure was blocked by three observability defects. The historical
chronology remains unchanged and is not being rewritten: the successful record
contained `request: null`, no `repo.rename-file` `tool_advertised` record, and
`marker_observed: false` even though `final_text` contained
`RAH_REPO_RENAME_FILE_LIVE_OK`.

This document records prospective Task 175A fixes only. No destructive live
rerun was performed.

## Source causes and fixes

1. `crates/rah-runtime-codex/src/bridge.rs` had a delete-specific request
   extractor. It returned `null` for every other public Tool, including
   `repo.rename-file`. The extractor now captures generic JSON input while
   applying bounded recursive redaction for sensitive keys and absolute paths.
   This preserves safe logical request fields such as the four rename
   precondition fields without persisting authority, credentials, environment,
   or opaque host identities.
2. The same bridge emitted `tool_advertised` only inside a
   `repo.delete-file` conditional. Advertisement evidence is now emitted for
   each dynamically emitted definition, retaining the canonical public name
   and the actual per-snapshot private alias.
3. `crates/rah-desktop/src/main.rs` checked only the hard-coded
   `RAH_REPO_DELETE_FILE_LIVE_OK` marker. Completion evidence now recognizes a
   complete marker in the diagnostic form `RAH_<NONEMPTY_UPPERCASE_TOKEN>_LIVE_OK`,
   including valid surrounding response text, while rejecting incomplete or
   misleading partial markers.

## Regression coverage

Deterministic tests cover generic rename request capture, exact safe fields,
sensitive-key and absolute-path redaction, request bounds, multiple
advertisements with canonical names and actual aliases, exact/contained/absent
and partial completion markers, and structured `ToolContent::Json` result
preservation including `renamed_verified`.

Existing bridge lifecycle, routing, replay, permission, and structured result
tests remain unchanged. The fixes do not alter Tool schemas, authority,
permissions, ToolRegistry routing, replay behavior, or Tool lifecycle counts.

## Validation boundary

This is observability-only hardening. No ADR, repository rename policy or
authority, Desktop authority construction, runtime semantics, dependency, or
release version changes are intended. Task 175's filesystem effect remains
historical single-effect evidence; no live rename was repeated.
