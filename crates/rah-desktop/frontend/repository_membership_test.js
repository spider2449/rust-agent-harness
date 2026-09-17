"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const html = fs.readFileSync(`${__dirname}/index.html`, "utf8");

assert.match(html, /id="repository-member-selector"/);
assert.match(html, /id="activate-repository-member"/);
assert.match(html, /id="remove-repository-member"/);
assert.match(html, /id="repository-member-removal-confirmation"/);
assert.match(html, /id="close-repository"[^>]*>Close Repository<\/button>/);
assert.match(html, /id="repository-close-confirmation"/);
assert.match(html, /Close the active repository\? It will remain admitted but inactive\. Repository files and remembered workspaces will not be deleted\. Disconnect first if a runtime is connected\./);
assert.match(html, /This removes only the current process-local workspace membership\. Repository files will not be deleted, and any remembered workspace entry will remain\./);
assert.match(source, /invoke\("repository_membership"\)/);
assert.match(source, /invoke\("activate_repository_member", \{ memberId: selector\.value \}\)/);
assert.match(source, /function renderRepositoryMembership\(/);
assert.match(source, /\(\$\{member\.active === true \? "Active" : "Inactive"\}\)/);
assert.match(source, /removeButton\.disabled = !selectedMember \|\| selectedIsActive/);
assert.match(source, /Switch to another repository before removing this active repository\./);
assert.match(source, /function installRepositoryMemberRemovalHandler\(/);
assert.match(source, /confirmation\.showModal\(\)/);
assert.match(source, /function removeRepositoryMember\(invoke, memberId\)/);
assert.match(source, /invoke\("remove_repository_member", \{ memberId \}\)/);
assert.match(source, /renderRepositoryMembership\(result\.membership\)/);
assert.match(source, /Repository removed from current workspace\./);
assert.match(source, /This repository is no longer in the current workspace\./);
assert.match(source, /This repository cannot be removed while related repository work is still in progress\./);
assert.match(source, /confirmation\.returnValue !== "confirm" \|\| !memberId/);
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
const removalHandler = source.slice(source.indexOf("function installRepositoryMemberRemovalHandler"), source.indexOf("function rememberedCandidateById"));
assert.equal(removalHandler.includes("activate_repository_member"), false);
assert.equal(removalHandler.includes("delete_remembered_workspace_candidate"), false);
assert.equal(removalHandler.includes("choose_repository"), false);
assert.equal(removalHandler.includes("admit_remembered_workspace_candidate"), false);
assert.equal(removalHandler.includes("repository_snapshot"), false);
assert.equal(removalHandler.includes("send_chat"), false);
assert.equal(removalHandler.includes("connect_codex"), false);
assert.equal(removalHandler.includes("localStorage"), false);
assert.equal(removalHandler.includes("sessionStorage"), false);
assert.equal(removalHandler.includes("innerHTML"), false);
const removalFunction = source.slice(source.indexOf("async function removeRepositoryMember"), source.indexOf("function installRepositoryMemberRemovalHandler"));
assert.match(removalFunction, /invoke\("remove_repository_member", \{ memberId \}\)/);
assert.equal(removalFunction.includes("path"), false);
assert.equal(removalFunction.includes("displayName"), false);
assert.match(source, /selector\.disabled = false/);
assert.equal(source.includes("delete_repository"), false);
assert.equal(source.includes("delete_repository_files"), false);

const closeGuard = source.slice(source.indexOf("function repositoryCloseGuard"), source.indexOf("function updateRepositoryCloseControls"));
assert.match(closeGuard, /renderedRepositoryMembership\?\.activeMemberId/);
assert.match(closeGuard, /member\.memberId === activeMemberId && member\.active === true/);
assert.match(closeGuard, /renderedEffectiveAuthority\?\.repository\?\.currentGeneration/);
assert.match(closeGuard, /Number\.isSafeInteger\(generation\)/);
assert.equal(closeGuard.includes("repository-member-selector"), false);
const closeFunction = source.slice(source.indexOf("async function closeActiveRepository"), source.indexOf("function installRepositoryCloseHandler"));
assert.match(closeFunction, /invoke\("close_repository", \{[\s\S]*request: \{[\s\S]*expectedActiveMemberId: capturedGuard\.expectedActiveMemberId,[\s\S]*expectedRepositoryGeneration: capturedGuard\.expectedRepositoryGeneration/);
assert.equal(closeFunction.includes("repository-member-selector"), false);
assert.equal(closeFunction.includes("activate_repository_member"), false);
assert.equal(closeFunction.includes("remove_repository_member"), false);
assert.equal(closeFunction.includes("disconnect_codex"), false);
assert.equal(closeFunction.includes("cancel_chat"), false);
assert.equal(closeFunction.includes("host_cancel_tool_invocation"), false);
assert.equal(closeFunction.includes("delete_remembered_workspace_candidate"), false);
assert.equal(closeFunction.includes("choose_repository"), false);
assert.equal(closeFunction.includes("clear_conversation_history"), false);
for (const forbidden of ["path:", "displayName:", "provider:", "connection:", "permission:", "localStorage", "sessionStorage", "indexedDB", "document.cookie"]) {
  assert.equal(closeFunction.includes(forbidden), false, `Close request or persistence field: ${forbidden}`);
}
const closeHandler = source.slice(source.indexOf("function installRepositoryCloseHandler"), source.indexOf("async function refreshRepositoryMembership"));
for (const forbidden of [
  "activate_repository_member",
  "remove_repository_member",
  "disconnect_codex",
  "delete_remembered_workspace_candidate",
  "update_remembered_workspace_candidate",
  "choose_repository",
  "cancel_chat",
  "host_cancel_tool_invocation",
  "localStorage",
  "sessionStorage",
  "innerHTML",
]) {
  assert.equal(closeHandler.includes(forbidden), false, `Close dialog isolation: ${forbidden}`);
}
const confirmHandler = closeHandler.slice(closeHandler.indexOf('confirmation.addEventListener("close"'));
assert.equal(confirmHandler.includes("repositoryCloseGuard()"), false);
assert.equal(confirmHandler.includes("renderedRepositoryMembership"), false);
assert.equal(confirmHandler.includes("renderedEffectiveAuthority"), false);
assert.match(source, /renderedEffectiveAuthority = snapshot\.status === "unavailable" \? null : snapshot/);
assert.match(source, /renderedEffectiveAuthority = null/);
assert.match(source, /function clearRepositorySnapshotPresentation\(/);
assert.match(source, /renderedCommitReview = null/);
assert.match(source, /async function refreshRepositoryCloseSuccessPresentation\(invoke\)/);
assert.match(source, /replaceTranscript\(invoke\)/);
assert.match(source, /Repository closed\. No repository is active\./);
assert.match(source, /active_changed: "The active repository changed\. Review the current repository before closing it\."/);
assert.match(source, /connected_or_runtime_busy: "Disconnect the runtime before closing the active repository\."/);
assert.match(source, /model_turn_busy: "Wait for the current chat turn to finish before closing the repository\."/);
assert.match(source, /repository_effect_busy: "Repository work is still in progress\. Finish or resolve it before closing the repository\."/);
assert.match(source, /invalid_guard: "Repository close state is stale\. Refresh and try again\."/);
assert.match(source, /unavailable: "Repository close is currently unavailable\."/);

class FakeElement {
  constructor(name) {
    this.name = name;
    this.listeners = new Map();
    this.children = [];
    this.dataset = {};
    this.disabled = false;
    this.hidden = false;
    this.value = "";
    this.returnValue = "";
  }

  addEventListener(type, handler) {
    const handlers = this.listeners.get(type) ?? [];
    handlers.push(handler);
    this.listeners.set(type, handlers);
  }

  dispatch(type, event = {}) {
    event.target ??= this;
    for (const handler of this.listeners.get(type) ?? []) handler(event);
  }

  showModal() {
    this.open = true;
  }

  replaceChildren(...children) {
    this.children = children;
  }

  append(...children) {
    this.children.push(...children);
  }
}

function createFrontend() {
  const elements = new Map();
  const document = {
    readyState: "loading",
    querySelector(selector) {
      if (!elements.has(selector)) elements.set(selector, new FakeElement(selector));
      return elements.get(selector);
    },
    createElement(name) {
      return new FakeElement(name);
    },
  };
  const window = { addEventListener() {} };
  const context = vm.createContext({ document, window });
  vm.runInContext(source, context, { filename: "status.js" });
  return {
    context,
    elements,
    run(code) {
      return vm.runInContext(code, context);
    },
  };
}

function membership(activeMemberId = "A") {
  return {
    activeMemberId,
    members: [
      { memberId: "A", displayName: "repo-a", active: activeMemberId === "A" },
      { memberId: "B", displayName: "repo-b", active: activeMemberId === "B" },
    ],
  };
}

function authority(generation = 17, status = "disconnected") {
  return {
    schemaVersion: 1,
    status,
    repository: { selected: true, displayName: "repo", identity: "current", currentGeneration: generation },
    connection: { state: "not_connected" },
    configured: {},
    effectiveTools: [],
    unavailableCapabilities: [],
  };
}

function configureFrontend(frontend, { activeMemberId = "A", generation = 17, codexStatus = "not connected", chat = false } = {}) {
  frontend.context.testMembership = membership(activeMemberId);
  frontend.context.testAuthority = authority(generation);
  frontend.context.testCodexStatus = codexStatus;
  frontend.context.testChatRunning = chat;
  frontend.run("renderRepositoryMembership(testMembership); renderedCodexStatus = testCodexStatus; chatRunning = testChatRunning; renderEffectiveAuthority(testAuthority)");
}

function assertCloseDisabled(frontend, codexStatus, hint) {
  configureFrontend(frontend, { codexStatus });
  assert.equal(frontend.elements.get("#close-repository").disabled, true, `${codexStatus} must block Close`);
  if (hint) assert.equal(frontend.elements.get("#repository-close-hint").textContent, hint);
}

const availability = createFrontend();
configureFrontend(availability);
assert.equal(availability.elements.get("#close-repository").disabled, false, "active member, generation and disconnected runtime enable Close");
assert.equal(availability.run("renderedEffectiveAuthority.repository.currentGeneration"), 17);
assert.equal(availability.run("renderedEffectiveAuthority === testAuthority"), true, "supported Effective Authority is retained as the rendered snapshot");
availability.elements.get("#repository-member-selector").value = "B";
availability.run("updateRepositoryMembershipControls()");
assert.equal(availability.run("repositoryCloseGuard().expectedActiveMemberId"), "A", "Close guard uses active A while B is selected");
assertCloseDisabled(availability, "connected", "Disconnect the runtime before closing the active repository.");
assertCloseDisabled(availability, "connecting", "The runtime is busy. Wait for it to finish before closing the active repository.");
assertCloseDisabled(availability, "disconnecting", "The runtime is busy. Wait for it to finish before closing the active repository.");
assertCloseDisabled(availability, "error", "Runtime status is unavailable. Resolve the connection state before closing the active repository.");
configureFrontend(availability, { chat: true });
assert.equal(availability.elements.get("#close-repository").disabled, true);
assert.equal(availability.elements.get("#repository-close-hint").textContent, "Wait for the current chat turn to finish before closing the repository.");
configureFrontend(availability);
availability.run("activeAssistant = {} ; updateRepositoryMembershipControls()");
assert.equal(availability.elements.get("#close-repository").disabled, true, "a visibly active assistant turn must block Close");
assert.equal(availability.elements.get("#repository-close-hint").textContent, "Wait for the current chat turn to finish before closing the repository.");
configureFrontend(availability, { activeMemberId: null });
assert.equal(availability.elements.get("#close-repository").disabled, true);
assert.equal(availability.elements.get("#repository-close-hint").textContent, "No repository is active.");
for (const generation of [0, null, "17", 1.5, Number.MAX_SAFE_INTEGER + 1]) {
  configureFrontend(availability, { generation });
  assert.equal(availability.elements.get("#close-repository").disabled, true, `generation ${String(generation)} must not enable Close`);
}
configureFrontend(availability);
availability.context.testUnsupportedAuthority = { schemaVersion: 0 };
availability.run("renderEffectiveAuthority(testUnsupportedAuthority)");
assert.equal(availability.run("renderedEffectiveAuthority"), null, "unsupported authority snapshots are unusable for Close");
assert.equal(availability.elements.get("#close-repository").disabled, true);
configureFrontend(availability);
availability.context.testUnavailableAuthority = authority(17, "unavailable");
availability.run("renderEffectiveAuthority(testUnavailableAuthority)");
assert.equal(availability.run("renderedEffectiveAuthority"), null, "unavailable authority snapshots are unusable for Close");
assert.equal(availability.elements.get("#close-repository").disabled, true);
configureFrontend(availability);
availability.context.testNoRepositoryAuthority = authority(17, "no_repository");
availability.context.testNoRepositoryAuthority.repository.selected = false;
availability.run("renderEffectiveAuthority(testNoRepositoryAuthority)");
assert.equal(availability.elements.get("#close-repository").disabled, true, "a generation without a selected repository is not a Close guard");
configureFrontend(availability);
availability.run("showBackendError()");
assert.equal(availability.elements.get("#close-repository").disabled, true, "unavailable app status must fail closed");

const staleCalls = [];
const staleFrontend = createFrontend();
configureFrontend(staleFrontend);
staleFrontend.elements.get("#repository-member-selector").value = "B";
staleFrontend.run("updateRepositoryMembershipControls()");
const pendingInvoke = (...args) => {
  staleCalls.push(args);
  return new Promise(() => {});
};
staleFrontend.context.testInvoke = pendingInvoke;
staleFrontend.run("installRepositoryCloseHandler(testInvoke)");
staleFrontend.elements.get("#close-repository").dispatch("click");
assert.equal(staleFrontend.elements.get("#repository-close-confirmation").open, true);
assert.deepEqual(JSON.parse(JSON.stringify(staleFrontend.run("pendingRepositoryClose"))), {
  expectedActiveMemberId: "A",
  expectedRepositoryGeneration: 17,
});
staleFrontend.context.testMembership = membership("B");
staleFrontend.context.testAuthority = authority(18);
staleFrontend.run("renderRepositoryMembership(testMembership); renderEffectiveAuthority(testAuthority)");
const staleDialog = staleFrontend.elements.get("#repository-close-confirmation");
staleDialog.returnValue = "confirm";
staleDialog.dispatch("close");
assert.equal(staleCalls.length, 1);
assert.equal(staleCalls[0][0], "close_repository");
assert.deepEqual(JSON.parse(JSON.stringify(staleCalls[0][1])), {
  request: {
    expectedActiveMemberId: "A",
    expectedRepositoryGeneration: 17,
  },
});
assert.deepEqual(Object.keys(staleCalls[0][1].request).sort(), ["expectedActiveMemberId", "expectedRepositoryGeneration"]);
assert.equal(staleFrontend.run("pendingRepositoryClose"), null);

const cancelCalls = [];
const cancelFrontend = createFrontend();
configureFrontend(cancelFrontend);
cancelFrontend.context.testInvoke = (...args) => cancelCalls.push(args);
cancelFrontend.run("installRepositoryCloseHandler(testInvoke)");
cancelFrontend.elements.get("#close-repository").dispatch("click");
const cancelDialog = cancelFrontend.elements.get("#repository-close-confirmation");
cancelDialog.returnValue = "cancel";
cancelDialog.dispatch("close");
assert.equal(cancelCalls.length, 0, "cancel must not invoke Close");
assert.equal(cancelFrontend.run("pendingRepositoryClose"), null);

async function testCloseSuccessAndErrors() {
const successFrontend = createFrontend();
configureFrontend(successFrontend);
let authorityRefreshes = 0;
let statusRefreshes = 0;
let transcriptRefreshes = 0;
successFrontend.context.testMembershipAfterClose = {
  activeMemberId: null,
  members: [
    { memberId: "A", displayName: "repo-a", active: false },
    { memberId: "B", displayName: "repo-b", active: false },
  ],
};
const successCalls = [];
successFrontend.context.testCalls = successCalls;
successFrontend.context.authorityRefresh = () => { authorityRefreshes += 1; };
successFrontend.context.statusRefresh = () => { statusRefreshes += 1; };
successFrontend.context.transcriptRefresh = () => { transcriptRefreshes += 1; };
successFrontend.run("refreshEffectiveAuthority = async () => authorityRefresh(); loadStatus = async () => statusRefresh(); replaceTranscript = async () => transcriptRefresh()");
successFrontend.context.testInvoke = async (...args) => {
  successCalls.push(args);
  return {
    outcome: "closed",
    status: "Repository closed. No repository is active.",
    membership: successFrontend.context.testMembershipAfterClose,
  };
};
await successFrontend.run("closeActiveRepository(testInvoke, { expectedActiveMemberId: 'A', expectedRepositoryGeneration: 17 })");
assert.equal(successCalls.length, 1);
assert.equal(successCalls[0][0], "close_repository");
assert.deepEqual(JSON.parse(JSON.stringify(successCalls[0][1])), {
  request: {
    expectedActiveMemberId: "A",
    expectedRepositoryGeneration: 17,
  },
});
assert.equal(successFrontend.run("renderedRepositoryMembership.activeMemberId"), null);
assert.equal(successFrontend.run("renderedRepositoryMembership.members.find((member) => member.memberId === 'A').active"), false);
assert.equal(successFrontend.elements.get("#repository-membership-status").textContent, "Repository closed. No repository is active.");
assert.equal(successFrontend.elements.get("#repository-path").textContent, "No repository is active.");
assert.equal(successFrontend.elements.get("#repository-status-entries").children[0].textContent, "No active repository");
assert.equal(successFrontend.elements.get("#worktree-diff-entries").children[0].textContent, "No active repository");
assert.equal(successFrontend.elements.get("#staged-diff-entries").children[0].textContent, "No active repository");
assert.equal(successFrontend.elements.get("#staged-review-state").textContent, "No repository is active.");
assert.equal(successFrontend.elements.get("#authorize-commit").hidden, true);
assert.equal(successFrontend.run("renderedCommitReview"), null);
assert.equal(successFrontend.elements.get("#repository-member-selector").disabled, false);
assert.equal(successFrontend.elements.get("#activate-repository-member").disabled, true);
assert.equal(successFrontend.elements.get("#close-repository").disabled, true);
assert.equal(authorityRefreshes, 1);
assert.equal(statusRefreshes, 1);
assert.equal(transcriptRefreshes, 1);
assert.equal(successFrontend.elements.get("#repository-status-entries").children.some((item) => item.children.some((child) => child.dataset?.repositoryAction)), false);

const errorMessages = {
  no_active: "No repository is active.",
  active_changed: "The active repository changed. Review the current repository before closing it.",
  connected_or_runtime_busy: "Disconnect the runtime before closing the active repository.",
  model_turn_busy: "Wait for the current chat turn to finish before closing the repository.",
  repository_effect_busy: "Repository work is still in progress. Finish or resolve it before closing the repository.",
  invalid_guard: "Repository close state is stale. Refresh and try again.",
  unavailable: "Repository close is currently unavailable.",
};
for (const [code, message] of Object.entries(errorMessages)) {
  const frontend = createFrontend();
  configureFrontend(frontend);
  let commandCount = 0;
  frontend.context.testInvoke = async (command) => {
    commandCount += 1;
    assert.equal(command, "close_repository");
    throw code;
  };
  frontend.run("refreshRepositoryCloseNoActivePresentation = async () => {}; refreshRepositoryCloseChangedPresentation = async () => {}");
  await frontend.run("closeActiveRepository(testInvoke, { expectedActiveMemberId: 'A', expectedRepositoryGeneration: 17 })");
  assert.equal(commandCount, 1, `${code} must not retry Close`);
  assert.equal(frontend.elements.get("#repository-error").textContent, message);
}

const changedFrontend = createFrontend();
configureFrontend(changedFrontend);
const changedRefreshes = { membership: 0, authority: 0, status: 0, transcript: 0, repository: 0 };
changedFrontend.context.changedRefreshes = changedRefreshes;
changedFrontend.context.testInvoke = async () => { throw "active_changed"; };
changedFrontend.run(`refreshRepositoryMembership = async () => { changedRefreshes.membership += 1 }; refreshEffectiveAuthority = async () => { changedRefreshes.authority += 1 }; loadStatus = async () => { changedRefreshes.status += 1 }; replaceTranscript = async () => { changedRefreshes.transcript += 1 }; refreshRepository = async () => { changedRefreshes.repository += 1 }`);
await changedFrontend.run("closeActiveRepository(testInvoke, { expectedActiveMemberId: 'A', expectedRepositoryGeneration: 17 })");
assert.deepEqual(changedRefreshes, { membership: 1, authority: 1, status: 1, transcript: 1, repository: 1 });
assert.equal(changedFrontend.elements.get("#repository-error").textContent, errorMessages.active_changed);

const noActiveFrontend = createFrontend();
configureFrontend(noActiveFrontend);
const noActiveRefreshes = { membership: 0, authority: 0, status: 0, transcript: 0, repository: 0 };
noActiveFrontend.context.noActiveRefreshes = noActiveRefreshes;
noActiveFrontend.context.testInvoke = async () => { throw "no_active"; };
noActiveFrontend.run(`refreshRepositoryMembership = async () => { noActiveRefreshes.membership += 1; renderRepositoryMembership({ activeMemberId: null, members: [{ memberId: 'A', displayName: 'repo-a', active: false }] }) }; refreshEffectiveAuthority = async () => { noActiveRefreshes.authority += 1 }; loadStatus = async () => { noActiveRefreshes.status += 1 }; replaceTranscript = async () => { noActiveRefreshes.transcript += 1 }; refreshRepository = async () => { noActiveRefreshes.repository += 1 }`);
await noActiveFrontend.run("closeActiveRepository(testInvoke, { expectedActiveMemberId: 'A', expectedRepositoryGeneration: 17 })");
assert.deepEqual(noActiveRefreshes, { membership: 1, authority: 1, status: 1, transcript: 1, repository: 0 });
assert.equal(noActiveFrontend.elements.get("#repository-membership-status").textContent, errorMessages.no_active);
assert.equal(noActiveFrontend.elements.get("#repository-path").textContent, "No repository is active.");
}

testCloseSuccessAndErrors().then(() => {
  console.log("repository membership frontend tests passed");
}).catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
