"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");

const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");

for (const status of [
  "no_repository",
  "disconnected",
  "connecting",
  "connected_current",
  "reconnect_required",
  "stale",
  "unavailable",
]) {
  assert.match(source, new RegExp(`${status}:`));
}

assert.match(source, /snapshot\.schemaVersion !== 1/);
assert.match(source, /Object\.hasOwn\(authorityStatusLabels, snapshot\.status\)/);
assert.match(source, /snapshot\.status === "connected_current"/);
assert.match(source, /renderEffectiveAuthority\(\{ schemaVersion: 0 \}\)/);
assert.match(source, /invoke\("get_effective_authority_snapshot"\)/);
assert.match(source, /function renderSourceLabel\(value\)/);
assert.match(source, /renderSourceLabel\(tool\.sourceLabel\)/);
assert.match(source, /repository_local_branch_creation: "Local branch creation"/);
assert.match(source, /authorityLabel\("authorityCategory", tool\.authorityCategory\)/);
assert.match(source, /tool\.hostInvocation/);
assert.match(source, /host\.eligible === true/);
assert.match(source, /host\.kind/);
for (const unavailableReason of [
  "not_connected_current",
  "repository_required",
  "permission_denied",
  "authority_not_granted",
  "model_turn_active",
  "host_invocation_busy",
  "stale",
  "not_supported",
]) {
  assert.match(source, new RegExp(`${unavailableReason}:`));
}
assert.match(source, /const isPatch = host\.kind === "repo_patch"/);
assert.match(source, /const isMultiFileEdit = host\.kind === "repo_edit_files"/);
assert.match(source, /const isCreateFile = host\.kind === "repo_create_file"/);
assert.match(source, /const isDeleteFile = host\.kind === "repo_delete_file"/);
assert.match(source, /const isRenameFile = host\.kind === "repo_rename_file"/);
assert.match(source, /input\.dataset\.hostInput = isPatch \? "path"/);
assert.match(source, /path\.dataset\.hostInput = "path"/);
assert.match(source, /fileContent\.dataset\.hostInput = "content"/);
assert.match(source, /fileContent\.maxLength = 262144/);
assert.match(source, /Prepare creates nothing/);
assert.match(source, /oldText\.dataset\.hostInput = "expectedOldText"/);
assert.match(source, /replacementText\.dataset\.hostInput = "replacementText"/);
assert.match(source, /host_prepare_repo_create_branch/);
assert.match(source, /host_prepare_repo_patch/);
assert.match(source, /host_prepare_repo_edit_files/);
assert.match(source, /host_prepare_repo_create_file/);
assert.match(source, /host_prepare_repo_rename_file/);
assert.match(source, /const isCreateFile = kind === "repo_create_file"/);
const createFileSubmit = source.slice(source.indexOf('const isCreateFile = kind === "repo_create_file"'), source.indexOf("const request = { kind }"));
assert.match(createFileSubmit, /path: form\.querySelector\('\[data-host-input="path"\]'\)\.value/);
assert.match(createFileSubmit, /content: form\.querySelector\('\[data-host-input="content"\]'\)\.value/);
for (const forbiddenCreateField of ["repository", "root", "nativePath", "ToolName", "ToolInput", "hash", "length", "permission", "authority", "retry", "stage", "commit", "ticketId", "activityId"]) {
  assert.equal(createFileSubmit.includes(forbiddenCreateField), false, `create-file DTO field: ${forbiddenCreateField}`);
}
const deleteForm = source.slice(source.indexOf("if (isDeleteFile)"), source.indexOf("if (isPatch)"));
assert.match(deleteForm, /path\.dataset\.hostInput = "path"/);
assert.match(deleteForm, /path\.required = true/);
assert.match(deleteForm, /path\.maxLength = 1024/);
assert.match(deleteForm, /Deletes exactly one reviewed existing tracked repository file/);
assert.match(deleteForm, /Prepare itself deletes nothing/);
assert.match(deleteForm, /no automatic Stage or Commit/);
assert.equal(deleteForm.includes("Delete Again"), false);
assert.equal(deleteForm.includes("Restore"), false);
assert.equal(deleteForm.includes('button.textContent = "Delete"'), false);
assert.equal(deleteForm.includes('button.textContent = "Restore"'), false);
assert.match(source, /button\.textContent = .*isDeleteFile.*\? "Prepare"/);
assert.match(source, /host_prepare_repo_delete_file/);
const deleteSubmit = source.slice(source.indexOf('if (kind === "repo_delete_file")'), source.indexOf("const request = { kind }"));
assert.match(deleteSubmit, /const request = \{\s*path: form\.querySelector\('\[data-host-input="path"\]'\)\.value,\s*\}/);
assert.match(deleteSubmit, /invoke\("host_prepare_repo_delete_file", \{ request \}\)/);
for (const forbiddenDeleteField of ["expected_file_sha256", "expected_file_byte_length", "hash", "length", "nativePath", "repository", "root", "FileIdentity", "permission", "authority", "ToolName", "ToolInput", "retry", "restore", "stage", "commit", "ticketId", "activityId"]) {
  assert.equal(deleteSubmit.includes(forbiddenDeleteField), false, `delete-file DTO field: ${forbiddenDeleteField}`);
}
const renameForm = source.slice(source.indexOf("if (isRenameFile)"), source.indexOf("if (isPatch)"));
assert.match(renameForm, /Source repository-relative path/);
assert.match(renameForm, /Destination repository-relative path/);
assert.equal((renameForm.match(/document\.createElement\("input"\)/g) ?? []).length, 2);
assert.match(renameForm, /source\.dataset\.hostInput = "sourcePath"/);
assert.match(renameForm, /destination\.dataset\.hostInput = "destinationPath"/);
assert.match(renameForm, /source\.maxLength = 1024/);
assert.match(renameForm, /destination\.maxLength = 1024/);
assert.match(renameForm, /Prepare itself renames nothing/);
assert.match(source, /button\.textContent = .*isRenameFile \? "Prepare"/);
const renameSubmit = source.slice(source.indexOf('if (kind === "repo_rename_file")'), source.indexOf("const request = { kind }"));
assert.match(renameSubmit, /const request = \{\s*source_path: form\.querySelector\('\[data-host-input="sourcePath"\]'\)\.value,\s*destination_path: form\.querySelector\('\[data-host-input="destinationPath"\]'\)\.value,\s*\}/);
assert.match(renameSubmit, /invoke\("host_prepare_repo_rename_file", \{ request \}\)/);
for (const forbiddenRenameField of ["expected_source_file_sha256", "expected_source_file_byte_length", "sourceSha256", "sourceByteLength", "sourceContent", "nativePath", "repository", "root", "ToolInput", "permission", "authority", "ticketId", "activityId"]) {
  assert.equal(renameSubmit.includes(forbiddenRenameField), false, `rename-file DTO field: ${forbiddenRenameField}`);
}
assert.equal(source.includes("createHash"), false);
assert.equal(source.includes("crypto."), false);
assert.equal(source.includes("window.__TAURI__.fs"), false);
assert.equal(source.includes("readFile"), false);
assert.match(source, /targets: \[\.\.\.form\.querySelectorAll\("\[data-multi-file-target\]"\)\]/);
assert.match(source, /expectedOldText: replacement\.querySelector/);
assert.match(source, /replacementText: replacement\.querySelector/);
assert.match(source, /multiFileMaxTargets = 4/);
assert.match(source, /multiFileMaxReplacements = 16/);
assert.match(source, /add-target/);
assert.match(source, /remove-target/);
assert.match(source, /add-replacement/);
assert.match(source, /remove-replacement/);
assert.match(source, /reset-draft/);
assert.match(source, /expectedOldText: form\.querySelector/);
assert.match(source, /replacementText: form\.querySelector/);
assert.match(source, /host_confirm_tool_invocation/);
assert.match(source, /request: \{ ticketId: active\.ticketId \}/);
assert.match(source, /confirmation\.addEventListener\("cancel"/);
assert.match(source, /event\.preventDefault\(\)/);
assert.match(source, /confirm\.disabled = true/);
assert.match(source, /cancel\.disabled = true/);
assert.match(source, /oldTextEscaped/);
assert.match(source, /replacementTextEscaped/);
assert.match(source, /target_count/);
assert.match(source, /replacement_count/);
assert.match(source, /changed_ranges/);
assert.match(source, /target_identity/);
assert.match(source, /preimage_sha256/);
assert.match(source, /postimage_sha256/);
assert.match(source, /non_atomic_warning/);
assert.match(source, /NON-ATOMIC/);
assert.match(source, /files execute in backend\/host order/);
for (const result of [
  "ok",
  "invalid_target",
  "precondition_failed",
  "failed_known_no_effect",
  "partial_effect",
  "uncertain",
]) {
  assert.match(source, new RegExp(`${result}:`));
}
assert.match(source, /committed_verified/);
assert.match(source, /unchanged_verified/);
assert.match(source, /not_attempted/);
assert.match(source, /Final effects cannot be fully determined/);
assert.match(source, /nothing was rolled back or continued/);
assert.match(source, /host_invocation_review_too_large/);
assert.match(source, /host_invocation_invalid_target/);
assert.match(source, /host_invocation_precondition_changed/);
assert.match(source, /host_invocation_stale/);
assert.match(source, /host_invocation_ticket_invalid/);
assert.match(source, /host_invocation_busy/);
assert.match(source, /showChatError\(error\)/);
assert.match(source, /pre\.textContent/);
assert.match(source, /Host action — not Model/);
assert.match(source, /content\?\.type !== "json"/);
assert.match(source, /renderHostResult\(payload\)/);
assert.equal(source.includes("renderHostOutput"), false);
assert.equal(source.includes("tool.name"), false);
assert.equal(source.includes("tool.publicToolName ==="), false);
assert.equal(source.includes("tool.permission ==="), false);
assert.equal(source.includes("tool.effectClass ==="), false);
assert.equal(source.includes("hostInvocation.eligible ="), false);
assert.match(source, /Remembered — not restored/);
assert.match(source, /No profile remembered/);
assert.match(source, /Configured — providers inactive/);
assert.match(source, /invoke\("restore_trusted_profile"\)/);
assert.match(source, /invoke\("forget_trusted_profile"\)/);
assert.match(source, /restore-trusted-profile/);
assert.match(source, /forget-trusted-profile/);
assert.match(source, /profileForgetAllowed/);
assert.match(source, /\["not connected", "error", "connected"\]/);
assert.equal(source.includes("Remembered"), true);
assert.equal(source.includes("Clear Profile"), false);

for (const forbidden of [
  "innerHTML",
  "insertAdjacentHTML",
  "outerHTML",
  "localStorage",
  "sessionStorage",
  "JSON.stringify",
  "toolName",
  "expectedSha256",
  "ticketId: request",
  'startsWith("repo.")',
  'includes("repo.")',
  "currentGeneration ===",
  "capturedGeneration ===",
  "capturedModelGeneration ===",
  "capturedConnectionGeneration ===",
]) {
  assert.equal(source.includes(forbidden), false, `forbidden authority pattern: ${forbidden}`);
}

const authorityRenderer = source.slice(source.indexOf("function renderEffectiveAuthority"), source.indexOf("async function refreshEffectiveAuthority"));
for (const field of [
  "schemaVersion",
  "status",
  "repository",
  "connection",
  "configured",
  "effectiveTools",
  "unavailableCapabilities",
  "reviewedCommit",
]) {
  assert.match(authorityRenderer, new RegExp(`snapshot\\.${field}`));
}

const hostActivityRenderer = source.slice(source.indexOf("function appendHostActivity"), source.indexOf("function renderHostReview"));
assert.equal(hostActivityRenderer.includes("payload.review"), false);
assert.equal(hostActivityRenderer.includes("preimage"), false);
assert.equal(hostActivityRenderer.includes("content_sha256"), false);
assert.equal(hostActivityRenderer.includes("content_byte_length"), false);
assert.match(source, /activePreparedHostReview/);
assert.match(source, /hostResultValue/);
assert.match(source, /ticketId: active\.ticketId/);
assert.match(source, /host_cancel_tool_invocation/);
assert.match(source, /void refreshEffectiveAuthority\(invoke\)/);
assert.match(source, /kind === "create_file"/);
assert.match(source, /kind === "delete_file"/);
for (const reviewField of [
  "target_count",
  "parent_path",
  "existing_parent",
  "target_worktree",
  "target_head",
  "target_index",
  "expected_effect",
  "content_escaped",
  "content_byte_length",
  "content_sha256",
  "content_facts",
  "file_intent",
  "creation_semantics",
  "non_effects",
  "warnings",
]) {
  assert.match(source, new RegExp(`review\\.${reviewField}`));
}
for (const contentFact of [
  "carriage_returns",
  "line_feeds",
  "crlf_pairs",
  "final_eof",
  "control_characters",
  "format_characters",
]) {
  assert.match(source, new RegExp(`facts\\.${contentFact}`));
}
assert.match(source, /contentHeading\.textContent = "Complete escaped new-file content"/);
assert.match(source, /contentPre\.textContent = String\(review\.content_escaped \?\? ""\)/);
assert.match(source, /Review Host new-file creation/);
assert.match(source, /request: \{ ticketId: active\.ticketId \}/);
assert.match(source, /\["tool_completed", "tool_error", "rejected_stale", "partial_effect", "possible_effect_unknown", "cancelled_before_start"\]/);

for (const result of [
  "ok",
  "invalid_target",
  "precondition_failed",
  "create_failed_known",
  "write_failed_known",
  "uncertain",
]) {
  assert.match(source, new RegExp(`${result}:`));
}
const createFileLabels = source.slice(source.indexOf("const createFileResultLabels"), source.indexOf("const authorityStatusLabels"));
assert.equal(createFileLabels.includes("all reviewed targets committed"), false);
assert.match(source, /write_failed_known[\s\S]*empty or partial file may remain/);
assert.match(source, /write_failed_known[\s\S]*No cleanup or retry\/replay occurred/);
assert.match(source, /status === "uncertain"[\s\S]*do not retry automatically/);
assert.match(source, /create_failed_known[\s\S]*no RAH creation effect/);
assert.match(source, /payload\.tool === "repo\.create-file"/);

const deleteReviewStart = source.indexOf('if (kind === "delete_file")');
const deleteReview = source.slice(deleteReviewStart, source.indexOf('\n  const details = document.createElement("dl")', deleteReviewStart));
for (const reviewField of [
  "operation",
  "target_count",
  "path",
  "tracked_state",
  "file_mode",
  "file_intent",
  "preimage_encoding",
  "preimage",
  "content_byte_length",
  "content_sha256",
  "bom",
  "head_blob_relationship",
  "index_relationship",
  "expected_effect",
  "post_delete_git_meaning",
  "non_effects",
  "warnings",
]) {
  assert.match(deleteReview, new RegExp(`review\\.${reviewField}`));
}
for (const contentFact of [
  "carriage_returns",
  "line_feeds",
  "crlf_pairs",
  "ends_with_newline",
  "final_eof",
  "contains_tab",
  "contains_trailing_space",
  "contains_control_or_format_escape",
  "control_characters",
  "format_characters",
  "empty",
]) {
  assert.match(deleteReview, new RegExp(`facts\\.${contentFact}`));
}
assert.match(source, /preimageHeading\.textContent = "Complete escaped file content to be deleted"/);
assert.match(source, /pre\.textContent = String\(review\.preimage \?\? ""\)/);
assert.match(source, /Review Host file deletion/);
const deleteLabels = source.slice(source.indexOf("const deleteFileResultLabels"), source.indexOf("const authorityStatusLabels"));
for (const result of [
  "deleted_verified",
  "known_no_effect",
  "invalid_input",
  "precondition_failed",
  "uncertain",
]) {
  assert.match(deleteLabels, new RegExp(`${result}:`));
}
assert.match(deleteLabels, /known_no_effect: "Delete failed — reviewed file proven unchanged"/);
assert.match(source, /status === "deleted_verified"[\s\S]*unstaged worktree deletion[\s\S]*No Stage or Commit/);
assert.match(source, /status === "known_no_effect"[\s\S]*exact reviewed file remains intact/);
assert.match(source, /status === "uncertain"[\s\S]*manual[\s\S]*Do not retry automatically/);
const deleteActivityRenderer = source.slice(source.indexOf("const deleteFileStatus"), source.indexOf("entry.append(title, state)"));
assert.match(deleteActivityRenderer, /payload\.tool === "repo\.delete-file"/);
assert.match(deleteActivityRenderer, /rejected_stale/);
assert.match(deleteActivityRenderer, /cancelled_before_start/);
assert.match(deleteActivityRenderer, /do not retry/);
assert.equal(deleteActivityRenderer.includes("Delete Again"), false);
assert.equal(deleteActivityRenderer.includes("Restore"), false);
assert.equal(deleteActivityRenderer.includes("Stage"), false);
assert.equal(deleteActivityRenderer.includes("Commit"), false);
assert.equal(deleteActivityRenderer.includes("activityId"), false);

const renameReviewStart = source.indexOf('if (kind === "rename_file")');
const renameReview = source.slice(renameReviewStart, source.indexOf("function clearActiveHostReview"));
for (const reviewField of [
  "operation",
  "source_path",
  "destination_path",
  "source_byte_length",
  "source_sha256",
  "source_format",
  "source_mode",
  "source_content_escaped",
  "expected_effect",
  "expected_git_consequence",
  "non_effects",
]) {
  assert.match(renameReview, new RegExp(`review\\.${reviewField}`));
}
assert.match(renameReview, /Source path/);
assert.match(renameReview, /Destination path/);
assert.match(renameReview, /Complete escaped source content/);
assert.match(renameReview, /contentPre\.textContent = String\(review\.source_content_escaped \?\? ""\)/);
assert.match(renameReview, /nonEffectsHeading\.textContent = "Explicit non-effects"/);
assert.match(source, /Review Host file rename \/ move/);
const renameLabels = source.slice(source.indexOf("const renameFileResultLabels"), source.indexOf("const authorityStatusLabels"));
for (const result of [
  "renamed_verified",
  "known_no_effect",
  "invalid_input",
  "precondition_failed",
  "uncertain",
]) {
  assert.match(renameLabels, new RegExp(`${result}:`));
}
assert.match(source, /status === "renamed_verified"[\s\S]*source is absent[\s\S]*destination matches[\s\S]*unstaged worktree rename\/move[\s\S]*no Stage or Commit/);
assert.match(source, /status === "known_no_effect"[\s\S]*source preimage remains intact[\s\S]*destination is absent/);
assert.match(source, /status === "uncertain"[\s\S]*does not automatically retry, reverse, roll back, or compensate/);
const renameActivityRenderer = source.slice(source.indexOf("const renameFileStatus"), source.indexOf("entry.append(title, state)"));
assert.match(renameActivityRenderer, /payload\.tool === "repo\.rename-file"/);
assert.equal(renameActivityRenderer.includes("source_content_escaped"), false);
assert.equal(renameActivityRenderer.includes("ticketId"), false);
assert.equal(renameActivityRenderer.includes("ToolOutput"), false);
assert.match(source, /if \(host\.eligible === true && host\.kind\)/);
assert.match(source, /host_invocation_busy: "HostExplicit busy"/);

const hostileReviewValues = [
  "<script>alert(1)</script>",
  "<img src=x onerror=alert(1)>",
  "</pre><script>alert(1)</script>",
  "<b>quoted & hostile</b>",
  "{\"expectedOldText\":\"secret\"}",
  "tauri://invoke('run_command')",
  "\u202Epath\u200Bwith\u0000controls",
  "quotes / braces / JSON-looking source",
  "\\u{202e}path\\u{200b}with\\u{0000}escapes",
];
function textNodeValue(value) {
  return String(value ?? "");
}
for (const hostile of hostileReviewValues) {
  assert.equal(textNodeValue(hostile), hostile);
}
assert.match(source, /pre\.textContent = String\(value \?\? ""\)/);
assert.match(source, /pre\.textContent = String\(review\.preimage \?\? ""\)/);
assert.equal(hostActivityRenderer.includes("textContent = payload"), false);

console.log("effective authority frontend static tests passed");
