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
