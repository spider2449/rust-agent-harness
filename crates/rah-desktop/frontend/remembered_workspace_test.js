"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");

const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const html = fs.readFileSync(`${__dirname}/index.html`, "utf8");
const styles = fs.readFileSync(`${__dirname}/styles.css`, "utf8");
const buildManifest = fs.readFileSync(`${__dirname}/../build.rs`, "utf8");
const capability = JSON.parse(fs.readFileSync(`${__dirname}/../capabilities/default.json`, "utf8"));

assert.match(html, /Remembered Workspaces/);
// The editor contains labels, a location picker, status, and actions. They must
// not inherit the single-row layout used by confirmation dialogs.
assert.match(source, /editor\.className = "remembered-editor"/);
assert.match(styles, /\.remembered-editor\s*\{[^}]*width:\s*min\(28rem, calc\(100vw - 2rem\)\)/s);
assert.match(styles, /\.remembered-editor\s*\{[^}]*max-height:\s*calc\(100dvh - 2rem\);[^}]*overflow-y:\s*auto/s);
assert.match(styles, /\.remembered-editor form\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/s);
assert.match(styles, /\.remembered-editor input\[type="checkbox"\]\s*\{[^}]*width:\s*auto/s);
assert.match(styles, /\.remembered-editor button\s*\{[^}]*max-width:\s*100%/s);
assert.match(html, /id="remembered-add-form"/);
assert.match(html, /id="remembered-catalog"/);
assert.match(html, /id="remembered-delete-confirmation"/);
assert.match(source, /invoke\("remembered_workspace_catalog"\)/);
assert.match(source, /function renderRememberedCatalog\(/);
assert.match(source, /hasLocationHint/);
assert.match(source, /Location saved — hidden/);
assert.match(source, /Show Location/);
assert.match(source, /Hide Location/);
assert.match(source, /invoke\("reveal_remembered_workspace_location"/);
assert.match(source, /invoke\("remember_workspace_candidate"/);
assert.match(source, /"update_remembered_workspace_candidate"/);
assert.match(source, /invoke\("delete_remembered_workspace_candidate"/);
assert.match(source, /invoke\("reorder_remembered_workspace_candidates"/);
assert.match(source, /invoke\("admit_remembered_workspace_candidate"/);
assert.match(source, /refreshRememberedCatalog\(invoke\)/);
assert.match(source, /result\.membership/);
assert.match(source, /renderRepositoryMembership\(result\.membership\)/);
const rememberedHandlers = source.slice(source.indexOf("function installRememberedWorkspaceHandlers"), source.indexOf("async function refreshRepository"));
assert.equal(rememberedHandlers.includes("activate_repository_member"), false);
assert.equal(source.includes("localStorage"), false);
assert.equal(source.includes("sessionStorage"), false);
assert.equal(source.includes("indexedDB"), false);
assert.equal(source.includes("innerHTML"), false);
assert.match(source, /remembered_catalog_unavailable/);
assert.match(source, /remembered_candidate_not_found/);
assert.match(source, /remembered_location_required/);
assert.match(source, /Remembered workspaces unavailable/);
assert.match(source, /setRememberedControlsDisabled\(true\)/);
assert.match(source, /revealedRememberedLocations\.delete\(candidateId\)/);
assert.match(source, /renderRememberedCatalog\(rememberedCatalog, \{ clearReveals: false \}\)/);
assert.equal(source.includes("candidate.location"), false);
assert.deepEqual(
  capability.permissions.filter((permission) => permission.startsWith("allow-") && (permission.includes("remembered") || permission === "allow-remember-workspace-candidate")),
  [
    "allow-remembered-workspace-catalog",
    "allow-reveal-remembered-workspace-location",
    "allow-remember-workspace-candidate",
    "allow-update-remembered-workspace-candidate",
    "allow-delete-remembered-workspace-candidate",
    "allow-reorder-remembered-workspace-candidates",
    "allow-admit-remembered-workspace-candidate",
  ],
);
assert.equal(capability.permissions.some((permission) => permission.includes("remembered-*")), false);
for (const [command, permissionFile, permission] of [
  ["remembered_workspace_catalog", "remembered_workspace_catalog.toml", "allow-remembered-workspace-catalog"],
  ["reveal_remembered_workspace_location", "reveal_remembered_workspace_location.toml", "allow-reveal-remembered-workspace-location"],
  ["remember_workspace_candidate", "remember_workspace_candidate.toml", "allow-remember-workspace-candidate"],
  ["update_remembered_workspace_candidate", "update_remembered_workspace_candidate.toml", "allow-update-remembered-workspace-candidate"],
  ["delete_remembered_workspace_candidate", "delete_remembered_workspace_candidate.toml", "allow-delete-remembered-workspace-candidate"],
  ["reorder_remembered_workspace_candidates", "reorder_remembered_workspace_candidates.toml", "allow-reorder-remembered-workspace-candidates"],
  ["admit_remembered_workspace_candidate", "admit_remembered_workspace_candidate.toml", "allow-admit-remembered-workspace-candidate"],
]) {
  assert.equal((buildManifest.match(new RegExp(`"${command}"`, "g")) ?? []).length, 1);
  const generated = fs.readFileSync(`${__dirname}/../permissions/autogenerated/${permissionFile}`, "utf8");
  assert.match(generated, new RegExp(`identifier = "${permission}"`));
  assert.equal(generated.includes(`commands.allow = ["${command}"]`), true);
  assert.equal(generated.includes(`commands.deny = ["${command}"]`), true);
}

console.log("remembered workspace frontend tests passed");
