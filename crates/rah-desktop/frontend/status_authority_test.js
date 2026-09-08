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
assert.match(source, /input\.dataset\.hostInput = isPatch \? "path"/);
assert.match(source, /oldText\.dataset\.hostInput = "expectedOldText"/);
assert.match(source, /replacementText\.dataset\.hostInput = "replacementText"/);
assert.match(source, /host_prepare_repo_create_branch/);
assert.match(source, /host_prepare_repo_patch/);
assert.match(source, /host_prepare_repo_edit_files/);
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
assert.match(source, /activePreparedHostReview/);
assert.match(source, /hostResultValue/);
assert.match(source, /ticketId: active\.ticketId/);
assert.match(source, /host_cancel_tool_invocation/);
assert.match(source, /void refreshEffectiveAuthority\(invoke\)/);

const hostileReviewValues = [
  "<script>alert(1)</script>",
  "<b>quoted & hostile</b>",
  "{\"expectedOldText\":\"secret\"}",
  "tauri://invoke('run_command')",
  "\\u202Epath\\u200Bwith\\u0000controls",
];
function textNodeValue(value) {
  return String(value ?? "");
}
for (const hostile of hostileReviewValues) {
  assert.equal(textNodeValue(hostile), hostile);
}
assert.match(source, /pre\.textContent = String\(value \?\? ""\)/);
assert.equal(hostActivityRenderer.includes("textContent = payload"), false);

console.log("effective authority frontend static tests passed");
