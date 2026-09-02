# Task 173 — Generic Structured Tool Result Live-Evidence Hardening

## Scope

Fix the generic live-evidence result extractor so structured `ToolContent::Json`
results are durably represented in future `tool_finished` JSONL evidence. This
is observability hardening only.

## Historical defect

Task 163 successfully executed `repo.delete-file`, but its historical evidence
recorded `tool_finished.result` as `null`. The historical record remains
unchanged and is not being rewritten.

## Source-level cause and fix

The extractor in `crates/rah-runtime-codex/src/bridge.rs` matched only exactly
one `ToolContent::Text` and returned `null` for every other content shape. The
generic extractor now preserves one structured JSON value, keeps valid JSON
text parsing and plain text compatibility, and represents multiple ordered
content items with bounded typed entries. Empty output remains `null`; oversized
values are represented by bounded truncation metadata.

The existing `RAH_LIVE_EVIDENCE_PATH` gate and best-effort write behavior are
unchanged. The evidence sink remains a sanitized observation sink rather than a
raw debug dump.

## Regression coverage

Focused tests cover structured rename-like JSON, delete-like JSON, plain text,
JSON-formatted text, empty output, and mixed multiple content items. The
rename-like test verifies `is_error = false` and the expected safe structured
fields in a `tool_finished` evidence shape.

## Boundary confirmation

No ADR, authority, policy, Tool input schema, permission, Trusted Profile,
Generic Bridge routing, Desktop authority, or Tool lifecycle semantics change.
Future `repo.rename-file` live validation can now durably capture its structured
result, but that live integration remains a separate task.
