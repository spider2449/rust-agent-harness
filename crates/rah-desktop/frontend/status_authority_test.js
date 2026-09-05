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

console.log("effective authority frontend static tests passed");
