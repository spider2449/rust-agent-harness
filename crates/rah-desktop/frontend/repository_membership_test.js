"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");

const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const html = fs.readFileSync(`${__dirname}/index.html`, "utf8");

assert.match(html, /id="repository-member-selector"/);
assert.match(html, /id="activate-repository-member"/);
assert.match(source, /invoke\("repository_membership"\)/);
assert.match(source, /invoke\("activate_repository_member", \{ memberId: selector\.value \}\)/);
assert.match(source, /renderRepositoryMembership\(result\.membership\)/);
assert.match(source, /result\.outcome === "activated"/);
assert.match(source, /selected === active/);
assert.match(source, /repositorySwitchBlocked/);
assert.match(source, /repository_member_selector_invalid/);
assert.match(source, /repository_member_not_found/);
assert.match(source, /repository_member_stale/);
assert.equal(source.includes("nativePath"), false);
assert.equal(source.includes("RepositoryAdmissionIdentity"), false);
assert.equal(source.includes("ToolRegistry"), false);
assert.equal(source.includes("repository_id"), false);
assert.equal(source.includes("invoke(\"activate_repository_member\", { path"), false);
assert.equal(source.includes("invoke(\"activate_repository_member\", { repository"), false);

console.log("repository membership frontend tests passed");
