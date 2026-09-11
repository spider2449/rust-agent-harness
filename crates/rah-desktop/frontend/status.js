const applicationRows = [
  ["Version", "appVersion"],
  ["Platform", "platform"],
  ["Desktop shell", "desktopShell"],
];

const runtimeRows = [
  ["RAH Runtime", "runtimeStatus"],
  ["Codex", "codexStatus"],
  ["Codex source", "codexSource"],
  ["Codex version", "codexVersion"],
  ["Profile", "profileStatus"],
  ["Repository", "repositoryStatus"],
  ["Repository tools", "repositoryToolsStatus"],
  ["Model configuration", "modelConfigurationStatus"],
];

let chatRunning = false;
let activeAssistant = null;
let resumeAvailable = false;
let resumeUsed = false;
let renderedModelConfiguration = null;
let renderedCommitReview = null;
let renderedTrustedProfileSelection = null;
let activePreparedHostReview = null;
const maxActivityEntries = 100;
const multiFileMaxTargets = 4;
const multiFileMaxReplacements = 16;

const multiFileResultLabels = {
  ok: "Verified — all reviewed targets committed",
  invalid_target: "Rejected — invalid target; no target effect",
  precondition_failed: "Rejected — precondition failed; no target effect",
  failed_known_no_effect: "Failed — stop target verified unchanged",
  partial_effect: "Partial effect — only the verified committed prefix is proven",
  uncertain: "Uncertain — final effects cannot be fully determined",
};

const createFileResultLabels = {
  ok: "Verified — new file created",
  invalid_target: "Rejected — invalid target; no creation performed",
  precondition_failed: "Rejected — creation precondition failed",
  create_failed_known: "Create failed — no RAH creation effect proven",
  write_failed_known: "Partial effect — new file may contain only a verified partial prefix",
  uncertain: "Uncertain — final file effect cannot be determined",
};

const deleteFileResultLabels = {
  deleted_verified: "Verified — reviewed file deleted",
  known_no_effect: "Delete failed — reviewed file proven unchanged",
  invalid_input: "Rejected — invalid deletion input; no deletion effect",
  precondition_failed: "Rejected — deletion precondition failed; no deletion effect",
  uncertain: "Uncertain — final file state cannot be proven",
};

const renameFileResultLabels = {
  renamed_verified: "Verified — reviewed file renamed / moved",
  known_no_effect: "Rename failed — reviewed source proven unchanged",
  invalid_input: "Rejected — invalid rename input; no rename effect",
  precondition_failed: "Rejected — rename precondition failed; no rename effect",
  uncertain: "Uncertain — final file location/state cannot be proven",
};

const authorityStatusLabels = {
  no_repository: "No repository selected",
  disconnected: "Runtime disconnected",
  connecting: "Connecting — effective runtime inventory pending",
  connected_current: "Current",
  reconnect_required: "Reconnect required",
  stale: "Stale — not current authority",
  unavailable: "Authority snapshot unavailable",
};

const authorityLabels = {
  sourceKind: { built_in: "Built-in", trusted_profile: "Trusted Profile", repository_host: "Repository host", mcp: "MCP", process_plugin: "Process Plugin" },
  repositoryIdentity: { current: "Current", not_selected: "Not selected", stale: "Stale", unknown: "Unknown / unavailable" },
  connectionState: { not_connected: "Runtime disconnected", connecting: "Connecting", connected: "Connected", disconnecting: "Disconnecting", error: "Unavailable" },
  effectClass: { read_only: "Read-only", repository_mutation: "Repository mutation", index_mutation: "Index mutation", commit: "Commit", execute: "Execute", external: "External" },
  authorityCategory: { repository_observation: "Repository observation", repository_content_mutation: "Content mutation", repository_file_creation: "File creation", repository_file_deletion: "File deletion", repository_file_rename: "File rename / move", repository_directory_creation: "Directory creation", repository_local_branch_creation: "Local branch creation", repository_index_mutation: "Index mutation", repository_commit: "Reviewed commit", read: "Read", execute: "Execute", external: "External provider" },
  permission: { none: "None", read: "Read", write: "Write", execute: "Execute" },
  sourceLabel: { desktop_builtin: "Desktop built-in", desktop_repository: "Desktop repository" },
  unavailableState: { configured_unavailable: "Configured unavailable", not_effective: "Not effective" },
  unavailableReason: { not_configured: "Not configured", authority_not_granted: "Host authority not granted", repository_required: "Select a repository", reconnect_required: "Reconnect required", provider_not_effective: "Provider not effective", provider_unavailable: "Provider unavailable", permission_not_configured: "Permission not configured", review_required: "Reviewed commit authorization required", stale_context: "Context is stale", not_connected_current: "Not connected/current", permission_denied: "Permission denied", model_turn_active: "Model turn active", host_invocation_busy: "HostExplicit busy", provider_not_supported: "Provider not supported for HostExplicit", not_supported: "Not supported", stale: "Stale — prepare again", unknown: "Unavailable reason unknown" },
  reviewedCommit: { not_applicable: "Not applicable", identity_not_configured: "Identity not configured", review_required: "Review required", ready_to_authorize: "Ready to authorize", authorized_pending: "Authorized pending", stale: "Stale", authorization_revoked: "Authorization revoked", unavailable: "Unavailable" },
};

const tauriApiRetryDelayMs = 100;
const tauriApiRetryAttempts = 20;

function setFrontendBootStatus(message) {
  document.querySelector("#frontend-boot-status").textContent = message;
}

function renderRows(element, rows, status) {
  element.replaceChildren(
    ...rows.filter(([, field]) => status[field]).map(([label, field]) => {
      const row = document.createElement("div");
      const term = document.createElement("dt");
      const detail = document.createElement("dd");

      term.textContent = label;
      detail.textContent = field === "codexSource" ? ({
        override: "Explicit override",
        certified_baseline: "Certified baseline",
        path: "PATH compatibility fallback",
      }[status[field]] ?? status[field]) : status[field];
      detail.dataset.status = status[field];
      row.append(term, detail);
      return row;
    }),
  );
}

function errorMessage(error) {
  const messages = {
    codex_not_found: "Codex executable not found",
    codex_baseline_invalid: "Certified Codex baseline is invalid",
    codex_host_unsupported: "Certified Codex baseline requires Windows x64",
    unsupported_codex_version: "Unsupported Codex version",
    codex_schema_incompatible: "Codex schema is incompatible",
    codex_start_failed: "Codex failed to start",
    codex_connection_failed: "Codex connection failed",
    tool_registry_failed: "Desktop tool registry unavailable",
    profile_invalid: "Trusted Profile is invalid or unsupported",
    profile_first_party_capabilities_unsupported: "Desktop v0.17 accepts provider-only Trusted Profiles; remove first-party capabilities",
    profile_activation_failed: "Trusted Profile providers could not be activated",
    profile_dialog_failed: "Trusted Profile picker failed",
    no_remembered_trusted_profile: "No Trusted Profile is remembered",
    trusted_profile_preference_save_failed: "Trusted Profile preference could not be saved",
    profile_busy: "Trusted Profile action is unavailable during the current activity",
    chat_empty_prompt: "Enter a message before sending",
    chat_prompt_too_large: "Message is too large",
    codex_not_connected: "Connect Codex to chat",
    chat_not_running: "No chat turn is running",
    chat_already_running: "A chat turn is already running",
    chat_start_failed: "Chat could not start",
    chat_runtime_failed: "Chat failed",
    chat_cancelled: "Chat was cancelled",
    conversation_context_limit: "Conversation context limit reached; start a new conversation context",
    conversation_history_busy: "Conversation history cannot be cleared while chat is running.",
    conversation_history_clear_failed: "Conversation history could not be cleared.",
    conversation_resume_unavailable: "Previous conversation is unavailable.",
    conversation_resume_busy: "Conversation cannot be resumed while chat is running.",
    conversation_resume_reconnect_required: "Reconnect Codex before resuming this conversation.",
    conversation_resume_too_large: "Previous conversation exceeds the replay limit. Start a new conversation context.",
    conversation_resume_persistence_failed: "Previous conversation could not be resumed.",
    conversation_resume_persistence_incompatible: "Previous conversation is unavailable.",
    git_unavailable: "Git is unavailable",
    repository_not_selected: "Choose a repository first",
    repository_invalid: "Selected folder is not a valid repository root",
    repository_observation_failed: "Repository observation failed",
    repository_dialog_failed: "Repository picker failed",
    repository_busy: "Repository selection is unavailable while chat is running",
    repository_action_invalid: "That repository action is no longer available. Refresh and choose it again.",
    repository_action_stale: "Repository changed after it was displayed. Refresh and choose a new action.",
    model_configuration_invalid: "Invalid model configuration",
    model_configuration_busy: "Model configuration is unavailable while chat is running",
    commit_identity_invalid: "Commit identity is invalid",
    commit_identity_save_failed: "Commit identity could not be saved",
    commit_authorization_unavailable: "Commit authorization is unavailable. Refresh and reconnect if needed.",
    commit_authorization_stale: "The staged review is stale. Refresh and review again.",
    commit_authorization_failed: "Commit authorization failed. Refresh and review again.",
    host_invocation_busy: "Host action is busy; wait for the current activity to finish.",
    host_invocation_not_connected: "Connect the current Desktop composition before using a Host action.",
    host_invocation_not_eligible: "This Tool is unavailable for Host action.",
    host_invocation_permission_denied: "The current host permission policy does not admit this action.",
    host_invocation_stale: "Host action is stale. Refresh Effective Authority and prepare again.",
    host_invocation_invalid_input: "The Host action input is invalid or too large.",
    host_invocation_ticket_invalid: "This Host action review is no longer valid. Prepare again.",
    host_invocation_invalid_target: "The prepared target is no longer valid. Prepare a fresh review.",
    host_invocation_precondition_changed: "A reviewed precondition changed. Prepare a fresh review.",
    host_invocation_review_too_large: "The complete backend review is too large to display safely.",
    preferences_save_failed: "Model preferences could not be saved.",
  };
  return messages[error] ?? "Desktop frontend unavailable";
}

function authorityLabel(group, value) {
  return authorityLabels[group][value] ?? "Unknown / unavailable";
}

function renderSourceLabel(value) {
  if (Object.hasOwn(authorityLabels.sourceLabel, value)) {
    return authorityLabels.sourceLabel[value];
  }
  if (typeof value === "string" && value.length > 0 && value.length <= 64 && /^[A-Za-z0-9._-]+$/.test(value)) {
    return value;
  }
  return "Unknown / unavailable";
}

function renderAuthorityValue(label, value) {
  const row = document.createElement("div");
  const term = document.createElement("dt");
  const detail = document.createElement("dd");
  term.textContent = label;
  detail.textContent = value;
  row.append(term, detail);
  return row;
}

function renderEffectiveAuthority(snapshot) {
  const statusElement = document.querySelector("#effective-authority-status");
  const error = document.querySelector("#effective-authority-error");
  const unsupportedSchema = !snapshot || snapshot.schemaVersion !== 1;
  const unsupportedStatus = !unsupportedSchema && !Object.hasOwn(authorityStatusLabels, snapshot.status);
  if (unsupportedSchema || unsupportedStatus) {
    statusElement.textContent = unsupportedSchema
      ? "Authority snapshot version unavailable"
      : "Authority snapshot unavailable";
    statusElement.dataset.state = "unavailable";
    error.hidden = true;
    document.querySelector("#effective-authority-summary").replaceChildren();
    document.querySelector("#effective-tools").replaceChildren();
    document.querySelector("#unavailable-capabilities").replaceChildren();
    document.querySelector("#effective-authority-advanced").replaceChildren();
    document.querySelector("#effective-authority-currentness-note").hidden = true;
    return;
  }
  const statusText = authorityStatusLabels[snapshot.status] ?? "Unknown / unavailable";
  statusElement.textContent = statusText;
  statusElement.dataset.state = snapshot.status === "connected_current" ? "current" : "unavailable";
  error.hidden = true;
  const repository = snapshot.repository ?? {};
  const connection = snapshot.connection ?? {};
  const configured = snapshot.configured ?? {};
  document.querySelector("#effective-authority-summary").replaceChildren(
    renderAuthorityValue("Status", statusText),
    renderAuthorityValue("Repository", repository.selected ? (repository.displayName ?? "Selected repository") : "No repository selected"),
    renderAuthorityValue("Binding", snapshot.status === "connected_current" && repository.identity === "current" ? "Current" : authorityLabel("repositoryIdentity", repository.identity) === "Current" ? "Not current" : authorityLabel("repositoryIdentity", repository.identity)),
    renderAuthorityValue("Runtime", connection.runtimeKind ?? authorityLabel("connectionState", connection.state)),
    renderAuthorityValue("Runtime source", connection.runtimeSource ?? "Unknown / unavailable"),
    renderAuthorityValue("Effective Tools", String((snapshot.effectiveTools ?? []).length)),
    renderAuthorityValue("Unavailable", String((snapshot.unavailableCapabilities ?? []).length)),
    renderAuthorityValue("Reviewed commit", authorityLabel("reviewedCommit", snapshot.reviewedCommit)),
  );
  document.querySelector("#effective-tools").replaceChildren(...(snapshot.effectiveTools?.length ? snapshot.effectiveTools.map(renderEffectiveTool) : [authorityListMessage("No effective Tools") ]));
  document.querySelector("#unavailable-capabilities").replaceChildren(...(snapshot.unavailableCapabilities?.length ? snapshot.unavailableCapabilities.map(renderUnavailableCapability) : [authorityListMessage("No known unavailable capabilities") ]));
  document.querySelector("#effective-authority-advanced").replaceChildren(
    renderAuthorityValue("Configured source", configured.profileSource ? authorityLabel("sourceKind", configured.profileSource) : "Unknown / unavailable"),
    renderAuthorityValue("Configured providers", String(configured.configuredProviderCount ?? 0)),
    renderAuthorityValue("Configured capabilities", String(configured.configuredCapabilityCount ?? 0)),
    renderAuthorityValue("Current repository generation", repository.currentGeneration == null ? "Not available" : String(repository.currentGeneration)),
    renderAuthorityValue("Captured repository generation", repository.capturedGeneration == null ? "Not available" : String(repository.capturedGeneration)),
    renderAuthorityValue("Captured model generation", connection.capturedModelGeneration == null ? "Not available" : String(connection.capturedModelGeneration)),
    renderAuthorityValue("Captured connection generation", connection.capturedConnectionGeneration == null ? "Not available" : String(connection.capturedConnectionGeneration)),
  );
  const currentnessNote = document.querySelector("#effective-authority-currentness-note");
  currentnessNote.textContent = "This runtime inventory is not current for the selected context.";
  currentnessNote.hidden = !["reconnect_required", "stale"].includes(snapshot.status);
}

function authorityListMessage(text) {
  const item = document.createElement("li");
  item.textContent = text;
  return item;
}

function renderEffectiveTool(tool) {
  const item = document.createElement("li");
  item.className = "authority-entry";
  const title = document.createElement("strong");
  title.textContent = tool.publicToolName;
  const details = document.createElement("dl");
  details.append(renderAuthorityValue("Source", `${authorityLabel("sourceKind", tool.sourceKind)} — ${renderSourceLabel(tool.sourceLabel)}`), renderAuthorityValue("Effect", authorityLabel("effectClass", tool.effectClass)), renderAuthorityValue("Authority", authorityLabel("authorityCategory", tool.authorityCategory)), renderAuthorityValue("Dispatch permission", `${authorityLabel("permission", tool.permission)} classification`), renderAuthorityValue("Repository bound", tool.repositoryBound === true ? "Yes" : "No"), renderAuthorityValue("Runtime", tool.advertised === true ? "Advertised" : "Not advertised / host effective only"));
  const host = tool.hostInvocation ?? { eligible: false, unavailableReason: "not_supported" };
  const hostBox = document.createElement("div");
  hostBox.className = "host-invocation";
  const hostLabel = document.createElement("span");
  hostLabel.textContent = host.eligible === true ? "Host action — not Model" : `Host action unavailable: ${authorityLabel("unavailableReason", host.unavailableReason ?? "not_supported")}`;
  hostBox.append(hostLabel);
  if (host.eligible === true && host.kind) {
    const form = document.createElement("form");
    form.dataset.hostKind = host.kind;
    const needsPath = ["fs_read", "repo_file_info"].includes(host.kind);
    const isBranch = host.kind === "repo_create_branch";
    const isPatch = host.kind === "repo_patch";
    const isMultiFileEdit = host.kind === "repo_edit_files";
    const isCreateFile = host.kind === "repo_create_file";
    const isDeleteFile = host.kind === "repo_delete_file";
    const isRenameFile = host.kind === "repo_rename_file";
    if (needsPath || isBranch || isPatch) {
      const input = document.createElement("input");
      input.type = "text";
      input.required = true;
      input.maxLength = isBranch ? 128 : 1024;
      input.placeholder = isBranch ? "Branch name" : "Relative path";
      input.dataset.hostInput = isPatch ? "path" : "value";
      form.append(input);
    }
    if (isCreateFile) {
      const pathLabel = document.createElement("label");
      pathLabel.textContent = "Repository-relative path";
      const path = document.createElement("input");
      path.type = "text";
      path.required = true;
      path.maxLength = 1024;
      path.placeholder = "Relative path";
      path.dataset.hostInput = "path";
      pathLabel.append(path);
      form.append(pathLabel);

      const contentLabel = document.createElement("label");
      contentLabel.textContent = "Complete new-file content";
      const fileContent = document.createElement("textarea");
      fileContent.maxLength = 262144;
      fileContent.placeholder = "Complete file content (may be empty)";
      fileContent.dataset.hostInput = "content";
      contentLabel.append(fileContent);
      form.append(contentLabel);

      const guidance = document.createElement("ul");
      for (const text of [
        "Creates exactly one previously absent repository file.",
        "The existing parent directory must already exist; existing files are never overwritten.",
        "Prepare creates nothing. The backend provides the complete review before Confirm.",
        "No automatic Stage or Commit is performed.",
      ]) {
        const item = document.createElement("li");
        item.textContent = text;
        guidance.append(item);
      }
      form.append(guidance);
    }
    if (isDeleteFile) {
      const pathLabel = document.createElement("label");
      pathLabel.textContent = "Repository-relative path";
      const path = document.createElement("input");
      path.type = "text";
      path.required = true;
      path.maxLength = 1024;
      path.placeholder = "Relative path";
      path.dataset.hostInput = "path";
      pathLabel.append(path);
      form.append(pathLabel);

      const guidance = document.createElement("ul");
      for (const text of [
        "Deletes exactly one reviewed existing tracked repository file.",
        "Prepare itself deletes nothing; the backend provides the complete exact preimage before Confirm.",
        "A verified deletion is an unstaged worktree deletion; no automatic Stage or Commit is performed.",
        "There is no Trash or Recycle Bin guarantee, backup, or restore.",
      ]) {
        const item = document.createElement("li");
        item.textContent = text;
        guidance.append(item);
      }
      form.append(guidance);
    }
    if (isRenameFile) {
      const sourceLabel = document.createElement("label");
      sourceLabel.textContent = "Source repository-relative path";
      const source = document.createElement("input");
      source.type = "text";
      source.required = true;
      source.maxLength = 1024;
      source.placeholder = "Relative source path";
      source.dataset.hostInput = "sourcePath";
      sourceLabel.append(source);
      form.append(sourceLabel);

      const destinationLabel = document.createElement("label");
      destinationLabel.textContent = "Destination repository-relative path";
      const destination = document.createElement("input");
      destination.type = "text";
      destination.required = true;
      destination.maxLength = 1024;
      destination.placeholder = "Relative destination path";
      destination.dataset.hostInput = "destinationPath";
      destinationLabel.append(destination);
      form.append(destinationLabel);

      const guidance = document.createElement("ul");
      for (const text of [
        "Prepares one reviewed tracked-file rename or move; Prepare itself renames nothing.",
        "The backend provides the complete review before Confirm.",
        "The operation is an unstaged worktree move with no automatic Stage or Commit.",
      ]) {
        const item = document.createElement("li");
        item.textContent = text;
        guidance.append(item);
      }
      form.append(guidance);
    }
    if (isPatch) {
      const oldText = document.createElement("textarea");
      oldText.required = true;
      oldText.maxLength = 64 * 1024;
      oldText.placeholder = "Expected old text";
      oldText.dataset.hostInput = "expectedOldText";
      const replacementText = document.createElement("textarea");
      replacementText.maxLength = 64 * 1024;
      replacementText.placeholder = "Replacement text (may be empty)";
      replacementText.dataset.hostInput = "replacementText";
      form.append(oldText, replacementText);
    }
    if (isMultiFileEdit) {
      form.classList.add("multi-file-edit-form");
      const guidance = document.createElement("p");
      guidance.textContent = "Typed literal replacements only. The host derives target order, hashes, postimages, and repository bindings.";
      form.append(guidance);
      const targets = document.createElement("div");
      targets.className = "multi-file-targets";
      targets.dataset.multiFileTargets = "true";
      appendMultiFileTarget(targets);
      form.append(targets);
      const addTarget = document.createElement("button");
      addTarget.type = "button";
      addTarget.dataset.multiFileAction = "add-target";
      addTarget.textContent = "Add target";
      form.append(addTarget);
      const reset = document.createElement("button");
      reset.type = "button";
      reset.dataset.multiFileAction = "reset-draft";
      reset.textContent = "Reset draft";
      form.append(reset);
      updateMultiFileBoundControls(form);
    }
    const button = document.createElement("button");
    button.type = "submit";
    button.textContent = isBranch || isPatch || isMultiFileEdit || isCreateFile || isDeleteFile || isRenameFile ? "Prepare" : "Invoke";
    if (isMultiFileEdit) button.dataset.multiFileAction = "prepare";
    form.append(button);
    hostBox.append(form);
  }
  item.append(hostBox);
  return item;
}

function appendMultiFileTarget(targets) {
  const target = document.createElement("fieldset");
  target.className = "multi-file-target";
  target.dataset.multiFileTarget = "true";
  const legend = document.createElement("legend");
  legend.textContent = "Editable target";
  target.append(legend);

  const pathLabel = document.createElement("label");
  pathLabel.textContent = "Relative file path";
  const path = document.createElement("input");
  path.type = "text";
  path.required = true;
  path.maxLength = 1024;
  path.dataset.multiFileInput = "path";
  pathLabel.append(path);
  target.append(pathLabel);

  const replacements = document.createElement("div");
  replacements.className = "multi-file-replacements";
  replacements.dataset.multiFileReplacements = "true";
  appendMultiFileReplacement(replacements);
  target.append(replacements);

  const addReplacement = document.createElement("button");
  addReplacement.type = "button";
  addReplacement.dataset.multiFileAction = "add-replacement";
  addReplacement.textContent = "Add replacement";
  target.append(addReplacement);

  const removeTarget = document.createElement("button");
  removeTarget.type = "button";
  removeTarget.dataset.multiFileAction = "remove-target";
  removeTarget.textContent = "Remove target";
  target.append(removeTarget);
  targets.append(target);
  updateMultiFileBoundControls(targets.closest("form"));
}

function appendMultiFileReplacement(replacements) {
  const replacement = document.createElement("fieldset");
  replacement.className = "multi-file-replacement";
  replacement.dataset.multiFileReplacement = "true";
  const legend = document.createElement("legend");
  legend.textContent = "Literal replacement";
  replacement.append(legend);

  const oldLabel = document.createElement("label");
  oldLabel.textContent = "Expected old text";
  const oldText = document.createElement("textarea");
  oldText.required = true;
  oldText.maxLength = 64 * 1024;
  oldText.dataset.multiFileInput = "expectedOldText";
  oldLabel.append(oldText);
  replacement.append(oldLabel);

  const newLabel = document.createElement("label");
  newLabel.textContent = "Replacement text";
  const newText = document.createElement("textarea");
  newText.maxLength = 64 * 1024;
  newText.dataset.multiFileInput = "replacementText";
  newLabel.append(newText);
  replacement.append(newLabel);
  replacements.append(replacement);
  updateMultiFileBoundControls(replacements.closest("form"));
}

function updateMultiFileBoundControls(form) {
  if (!form) return;
  const targets = [...form.querySelectorAll("[data-multi-file-target]")];
  form.querySelector('[data-multi-file-action="add-target"]').disabled = targets.length >= multiFileMaxTargets;
  for (const target of targets) {
    const replacements = [...target.querySelectorAll("[data-multi-file-replacement]")];
    target.querySelector('[data-multi-file-action="add-replacement"]').disabled = replacements.length >= multiFileMaxReplacements;
    target.querySelector('[data-multi-file-action="remove-target"]').disabled = targets.length <= 1;
    for (const replacement of replacements) {
      let remove = replacement.querySelector('[data-multi-file-action="remove-replacement"]');
      if (!remove) {
        remove = document.createElement("button");
        remove.type = "button";
        remove.dataset.multiFileAction = "remove-replacement";
        remove.textContent = "Remove replacement";
        replacement.append(remove);
      }
      remove.disabled = replacements.length <= 1;
    }
  }
}

function resetMultiFileDraft(form) {
  const targets = form.querySelector("[data-multi-file-target]").parentElement;
  targets.replaceChildren();
  appendMultiFileTarget(targets);
  updateMultiFileBoundControls(form);
}

function readMultiFileRequest(form) {
  return {
    targets: [...form.querySelectorAll("[data-multi-file-target]")].map((target) => ({
      path: target.querySelector('[data-multi-file-input="path"]').value,
      replacements: [...target.querySelectorAll("[data-multi-file-replacement]")].map((replacement) => ({
        expectedOldText: replacement.querySelector('[data-multi-file-input="expectedOldText"]').value,
        replacementText: replacement.querySelector('[data-multi-file-input="replacementText"]').value,
      })),
    })),
  };
}

function hostResultValue(output) {
  if (!output || !Array.isArray(output.content) || output.content.length !== 1) return null;
  const content = output.content[0];
  if (content?.type !== "json" || !content.value || typeof content.value !== "object") return null;
  return content.value;
}

function renderHostResult(payload) {
  const result = hostResultValue(payload.result);
  const status = result?.status;
  const isCreateFile = payload.tool === "repo.create-file";
  const isDeleteFile = payload.tool === "repo.delete-file";
  const isRenameFile = payload.tool === "repo.rename-file";
  const resultLabels = isCreateFile ? createFileResultLabels : isDeleteFile ? deleteFileResultLabels : isRenameFile ? renameFileResultLabels : multiFileResultLabels;
  const box = document.createElement("div");
  box.className = "host-result";
  const title = document.createElement("strong");
  title.textContent = resultLabels[status] ?? (payload.state === "possible_effect_unknown"
    ? "Uncertain — backend result was not safely classifiable"
    : "Host result unavailable");
  box.append(title);
  if (!resultLabels[status]) return box;

  if (isCreateFile) {
    const explanation = document.createElement("p");
    if (status === "ok") {
      explanation.textContent = "The backend verified the reviewed new-file creation. No Stage or Commit was performed.";
    } else if (status === "invalid_target") {
      explanation.textContent = "The backend rejected the target before creation; no creation was performed.";
    } else if (status === "precondition_failed") {
      explanation.textContent = "The backend rejected a required creation precondition; no successful creation is reported.";
    } else if (status === "create_failed_known") {
      explanation.textContent = "The backend proved that this attempt produced no RAH creation effect. This is distinct from partial or uncertain effect.";
    } else if (status === "write_failed_known") {
      explanation.textContent = "Exclusive create may already have succeeded. An empty or partial file may remain; this is not a known-no-effect failure. No cleanup or retry/replay occurred. Inspect the current repository state.";
    } else if (status === "uncertain") {
      explanation.textContent = "The final target state may be unknown. Do not infer unchanged state; do not retry automatically. Inspect the refreshed repository state manually.";
    }
    box.append(explanation);
    return box;
  }

  if (isDeleteFile) {
    const explanation = document.createElement("p");
    if (status === "deleted_verified") {
      explanation.textContent = "The backend independently verified that the reviewed target is absent. The deletion is an unstaged worktree deletion. No Stage or Commit was performed.";
    } else if (status === "known_no_effect") {
      explanation.textContent = "The deletion attempt did not produce a RAH deletion effect and the backend independently proved that the exact reviewed file remains intact.";
    } else if (status === "invalid_input") {
      explanation.textContent = "The backend rejected the deletion request as invalid; no verified deletion effect occurred.";
    } else if (status === "precondition_failed") {
      explanation.textContent = "A required deletion precondition failed; no verified deletion effect occurred. Prepare a fresh review if the human still intends to delete the current file.";
    } else if (status === "uncertain") {
      explanation.textContent = "The final target state cannot be safely proven. Inspect the refreshed repository state manually. Do not retry automatically.";
    }
    box.append(explanation);
    return box;
  }

  if (isRenameFile) {
    const explanation = document.createElement("p");
    if (status === "renamed_verified") {
      explanation.textContent = "The backend independently proved that the reviewed source is absent and the destination matches the reviewed source. This is an unstaged worktree rename/move; no Stage or Commit was performed.";
    } else if (status === "known_no_effect") {
      explanation.textContent = "The backend independently proved that the reviewed source preimage remains intact and the destination is absent. No rename effect is reported.";
    } else if (status === "invalid_input") {
      explanation.textContent = "The backend rejected the rename request as invalid; no verified rename effect occurred.";
    } else if (status === "precondition_failed") {
      explanation.textContent = "A required rename precondition failed; no verified rename effect occurred. Prepare a fresh review if the human still intends to move the file.";
    } else if (status === "uncertain") {
      explanation.textContent = "The final file location or state cannot be proven. RAH does not automatically retry, reverse, roll back, or compensate; inspect the refreshed repository state manually.";
    }
    box.append(explanation);
    return box;
  }

  const effects = Array.isArray(result.effects) ? result.effects : [];
  if (effects.length) {
    const heading = document.createElement("p");
    heading.textContent = `Verified target effects: ${effects.length}`;
    box.append(heading);
    const list = document.createElement("ul");
    for (const effect of effects) {
      const item = document.createElement("li");
      const state = ["committed_verified", "unchanged_verified", "not_attempted", "uncertain"].includes(effect?.state)
        ? effect.state
        : "unclassifiable";
      item.textContent = `Backend-proven target effect state: ${state}`;
      list.append(item);
    }
    box.append(list);
  }
  const explanation = document.createElement("p");
  if (status === "partial_effect") {
    explanation.textContent = "Only the verified committed prefix/effect subset is proven. Later targets are shown only with backend-proven effect states; nothing was rolled back or continued.";
  } else if (status === "uncertain") {
    explanation.textContent = "Final effects cannot be fully determined. The frontend does not infer unchanged state and offers no automatic Retry or Continue.";
  } else {
    explanation.textContent = "No automatic retry, replay, continuation, rollback, restore-preimage, Stage, or Commit is performed.";
  }
  box.append(explanation);
  return box;
}

function appendHostActivity(payload) {
  const entries = document.querySelector("#activity-entries");
  const entry = document.createElement("article");
  const title = document.createElement("strong");
  const state = document.createElement("span");
  const labels = {
    prepared: "Host action prepared",
    started: "Host action started",
    tool_completed: "Host action completed",
    tool_error: "Host action failed",
    rejected_not_eligible: "Host action unavailable",
    rejected_permission: "Host action denied",
    rejected_stale: "Host action stale",
    rejected_busy: "Host action busy",
    invalid_input: "Host input invalid",
    cancelled_before_start: "Host action cancelled before start",
    partial_effect: "Host action partially effected",
    possible_effect_unknown: "Host effect outcome unknown — inspect manually; do not retry",
  };
  entry.className = "activity-entry host-activity";
  entry.dataset.state = payload.state;
  title.textContent = "Host action — not Model";
  const result = hostResultValue(payload.result);
  const createFileStatus = payload.tool === "repo.create-file" ? result?.status : null;
  const createFileLabel = createFileStatus ? createFileResultLabels[createFileStatus] : null;
  const deleteFileStatus = payload.tool === "repo.delete-file" ? result?.status : null;
  const deleteFileLabel = deleteFileStatus ? deleteFileResultLabels[deleteFileStatus] : null;
  const renameFileStatus = payload.tool === "repo.rename-file" ? result?.status : null;
  const renameFileLabel = renameFileStatus ? renameFileResultLabels[renameFileStatus] : null;
  const activityLabel = payload.tool === "repo.create-file"
    ? (createFileLabel ?? (payload.state === "partial_effect"
      ? "Partial effect — inspect current repository state"
      : payload.state === "possible_effect_unknown"
        ? "Uncertain — inspect current repository state; do not retry"
        : payload.state === "cancelled_before_start"
          ? "Cancelled without effect"
          : labels[payload.state] ?? "Host action state unavailable"))
    : payload.tool === "repo.delete-file"
      ? (deleteFileLabel ?? (payload.state === "possible_effect_unknown"
        ? "Uncertain — inspect the refreshed repository state manually; do not retry"
        : payload.state === "cancelled_before_start"
          ? "Cancelled without effect"
          : payload.state === "rejected_stale"
            ? "Rejected stale — no deletion effect from this review"
            : labels[payload.state] ?? "Host action state unavailable"))
      : payload.tool === "repo.rename-file"
        ? (renameFileLabel ?? (payload.state === "possible_effect_unknown"
          ? "Uncertain — inspect the refreshed repository state manually; do not retry"
          : payload.state === "cancelled_before_start"
            ? "Cancelled without effect"
            : payload.state === "rejected_stale"
              ? "Rejected stale — no rename effect from this review"
              : labels[payload.state] ?? "Host action state unavailable"))
    : labels[payload.state] ?? "Host action state unavailable";
  state.textContent = `${payload.tool}: ${activityLabel}`;
  entry.append(title, state);
  if (payload.result) entry.append(renderHostResult(payload));
  if (["tool_completed", "tool_error", "rejected_stale", "partial_effect", "possible_effect_unknown", "cancelled_before_start"].includes(payload.state)) {
    clearActiveHostReview();
  }
  entries.append(entry);
  while (entries.children.length > maxActivityEntries) entries.firstElementChild.remove();
  entries.scrollTop = entries.scrollHeight;
}

function renderHostReview(review, kind) {
  const content = document.createElement("div");
  content.className = "host-review-content";
  if (kind === "branch") {
    const text = document.createElement("p");
    text.textContent = `Create branch only: ${review.branch}. Does not switch branch.`;
    content.append(text);
    return content;
  }
  if (kind === "multi_file_edit") {
    const details = document.createElement("dl");
    const values = [
      ["Operation", review.operation],
      ["Target count", review.target_count],
      ["Total replacement count", review.replacement_count],
      ["Matching semantics", review.matching],
      ["Unchanged context", review.unchanged_context],
      ["Intended effect", review.intended_effect],
      ["Non-atomic warning", review.non_atomic_warning],
    ];
    for (const [label, value] of values) {
      const term = document.createElement("dt");
      const detail = document.createElement("dd");
      term.textContent = label;
      detail.textContent = String(value ?? "");
      details.append(term, detail);
    }
    content.append(details);

    const warning = document.createElement("p");
    warning.className = "host-review-warning";
    warning.textContent = "NON-ATOMIC: files execute in backend/host order, not input order. partial_effect and uncertain outcomes are possible.";
    content.append(warning);

    const targetsHeading = document.createElement("h4");
    targetsHeading.textContent = "Host-ordered targets and complete changed ranges";
    content.append(targetsHeading);
    for (const target of review.targets ?? []) {
      const targetSection = document.createElement("section");
      targetSection.className = "multi-file-review-target";
      const heading = document.createElement("h5");
      heading.textContent = `Host target ordinal ${target.ordinal}: ${target.path}`;
      targetSection.append(heading);
      const targetDetails = document.createElement("dl");
      for (const [label, value] of [
        ["Target ordinal", target.ordinal],
        ["Target path", target.path],
        ["Target identity", target.target_identity],
        ["Replacement count", target.replacement_count],
        ["Preimage SHA-256", target.preimage_sha256],
        ["Preimage byte length", target.preimage_byte_length],
        ["Postimage SHA-256", target.postimage_sha256],
        ["Postimage byte length", target.postimage_byte_length],
      ]) {
        const term = document.createElement("dt");
        const detail = document.createElement("dd");
        term.textContent = label;
        detail.textContent = String(value ?? "");
        targetDetails.append(term, detail);
      }
      targetSection.append(targetDetails);

      const rangesHeading = document.createElement("h6");
      rangesHeading.textContent = "Changed ranges and complete changed material";
      targetSection.append(rangesHeading);
      for (const [rangeIndex, range] of (target.changed_ranges ?? []).entries()) {
        const rangeSection = document.createElement("section");
        const rangeHeading = document.createElement("strong");
        rangeHeading.textContent = `Changed range ${rangeIndex}: [${range.start}, ${range.end}) length ${range.length}`;
        rangeSection.append(rangeHeading);
        for (const [label, value] of [
          ["Complete escaped expected old text", range.expected_old_text_escaped],
          ["Complete escaped replacement text", range.replacement_text_escaped],
        ]) {
          const textHeading = document.createElement("strong");
          const pre = document.createElement("pre");
          textHeading.textContent = label;
          pre.textContent = String(value ?? "");
          rangeSection.append(textHeading, pre);
        }
        targetSection.append(rangeSection);
      }
      content.append(targetSection);
    }

    const nonEffectsHeading = document.createElement("strong");
    nonEffectsHeading.textContent = "Protected non-effects";
    const nonEffects = document.createElement("ul");
    for (const value of review.non_effects ?? []) {
      const item = document.createElement("li");
      item.textContent = value;
      nonEffects.append(item);
    }
    content.append(nonEffectsHeading, nonEffects);
    const lifecycleWarning = document.createElement("p");
    lifecycleWarning.textContent = "There is no automatic retry, replay, continuation of remaining targets, rollback, restore-preimage, Stage, or Commit.";
    content.append(lifecycleWarning);
    return content;
  }
  if (kind === "create_file") {
    const details = document.createElement("dl");
    const values = [
      ["Operation", review.operation],
      ["Target count", review.target_count],
      ["Relative path", review.path],
      ["Parent path", review.parent_path],
      ["Existing parent status", review.existing_parent],
      ["Worktree target status", review.target_worktree],
      ["HEAD target status", review.target_head],
      ["Index target status", review.target_index],
      ["Expected effect", review.expected_effect],
      ["Content byte length", review.content_byte_length],
      ["Content SHA-256", review.content_sha256],
      ["BOM state", review.bom],
      ["File intent", review.file_intent],
    ];
    for (const [label, value] of values) {
      const term = document.createElement("dt");
      const detail = document.createElement("dd");
      term.textContent = label;
      detail.textContent = String(value ?? "");
      details.append(term, detail);
    }
    content.append(details);

    const contentHeading = document.createElement("h4");
    contentHeading.textContent = "Complete escaped new-file content";
    const contentPre = document.createElement("pre");
    contentPre.textContent = String(review.content_escaped ?? "");
    content.append(contentHeading, contentPre);

    const factsHeading = document.createElement("h4");
    factsHeading.textContent = "Content visibility facts";
    const facts = review.content_facts ?? {};
    const factsDetails = document.createElement("dl");
    for (const [label, value] of [
      ["CR count", facts.carriage_returns],
      ["LF count", facts.line_feeds],
      ["CRLF count", facts.crlf_pairs],
      ["Final EOF/newline state", facts.final_eof],
      ["Control-character count", facts.control_characters],
      ["Format/bidi/zero-width count", facts.format_characters],
    ]) {
      const term = document.createElement("dt");
      const detail = document.createElement("dd");
      term.textContent = label;
      detail.textContent = String(value ?? "");
      factsDetails.append(term, detail);
    }
    content.append(factsHeading, factsDetails);

    const semanticsHeading = document.createElement("h4");
    semanticsHeading.textContent = "Creation semantics";
    const semantics = document.createElement("ul");
    for (const value of review.creation_semantics ?? []) {
      const item = document.createElement("li");
      item.textContent = String(value ?? "");
      semantics.append(item);
    }
    content.append(semanticsHeading, semantics);

    const nonEffectsHeading = document.createElement("h4");
    nonEffectsHeading.textContent = "Explicit non-effects";
    const nonEffects = document.createElement("ul");
    for (const value of review.non_effects ?? []) {
      const item = document.createElement("li");
      item.textContent = String(value ?? "");
      nonEffects.append(item);
    }
    content.append(nonEffectsHeading, nonEffects);

    const warningsHeading = document.createElement("h4");
    warningsHeading.textContent = "Warnings before Confirm";
    const warnings = document.createElement("ul");
    warnings.className = "host-review-warning";
    for (const value of review.warnings ?? []) {
      const item = document.createElement("li");
      item.textContent = String(value ?? "");
      warnings.append(item);
    }
    content.append(warningsHeading, warnings);
    return content;
  }
  if (kind === "delete_file") {
    const details = document.createElement("dl");
    const values = [
      ["Operation", review.operation],
      ["Target count", review.target_count],
      ["Relative path", review.path],
      ["Tracked state", review.tracked_state],
      ["File mode", review.file_mode],
      ["File intent", review.file_intent],
      ["Preimage encoding", review.preimage_encoding],
      ["Content byte length", review.content_byte_length],
      ["Content SHA-256", review.content_sha256],
      ["BOM state", review.bom],
      ["HEAD/blob relationship", review.head_blob_relationship],
      ["Index relationship", review.index_relationship],
      ["Expected effect", review.expected_effect],
      ["Post-delete Git meaning", review.post_delete_git_meaning],
    ];
    for (const [label, value] of values) {
      const term = document.createElement("dt");
      const detail = document.createElement("dd");
      term.textContent = label;
      detail.textContent = String(value ?? "");
      details.append(term, detail);
    }
    content.append(details);

    const preimageHeading = document.createElement("h4");
    preimageHeading.textContent = "Complete escaped file content to be deleted";
    const pre = document.createElement("pre");
    pre.textContent = String(review.preimage ?? "");
    content.append(preimageHeading, pre);

    const factsHeading = document.createElement("h4");
    factsHeading.textContent = "Content facts";
    const facts = review.content_facts ?? {};
    const factsDetails = document.createElement("dl");
    for (const [label, value] of [
      ["Carriage returns", facts.carriage_returns],
      ["Line feeds", facts.line_feeds],
      ["CRLF pairs", facts.crlf_pairs],
      ["Contains CR", facts.contains_cr],
      ["Contains LF", facts.contains_lf],
      ["Ends with newline", facts.ends_with_newline],
      ["Final EOF", facts.final_eof],
      ["Contains tab", facts.contains_tab],
      ["Contains trailing space", facts.contains_trailing_space],
      ["Contains control or format escape", facts.contains_control_or_format_escape],
      ["Control characters", facts.control_characters],
      ["Format characters", facts.format_characters],
      ["Empty", facts.empty],
    ]) {
      const term = document.createElement("dt");
      const detail = document.createElement("dd");
      term.textContent = label;
      detail.textContent = String(value ?? "");
      factsDetails.append(term, detail);
    }
    content.append(factsHeading, factsDetails);

    const nonEffectsHeading = document.createElement("h4");
    nonEffectsHeading.textContent = "Explicit non-effects";
    const nonEffects = document.createElement("ul");
    for (const value of review.non_effects ?? []) {
      const item = document.createElement("li");
      item.textContent = String(value ?? "");
      nonEffects.append(item);
    }
    content.append(nonEffectsHeading, nonEffects);

    const warningsHeading = document.createElement("h4");
    warningsHeading.textContent = "Warnings before Confirm";
    const warnings = document.createElement("ul");
    warnings.className = "host-review-warning";
    for (const value of review.warnings ?? []) {
      const item = document.createElement("li");
      item.textContent = String(value ?? "");
      warnings.append(item);
    }
    content.append(warningsHeading, warnings);
    return content;
  }
  if (kind === "rename_file") {
    const details = document.createElement("dl");
    const values = [
      ["Operation", review.operation],
      ["Source path", review.source_path],
      ["Destination path", review.destination_path],
      ["Source byte length", review.source_byte_length],
      ["Source SHA-256", review.source_sha256],
      ["Source format", review.source_format],
      ["Source mode", review.source_mode],
      ["Expected effect", review.expected_effect],
      ["Expected Git consequence", review.expected_git_consequence],
    ];
    for (const [label, value] of values) {
      const term = document.createElement("dt");
      const detail = document.createElement("dd");
      term.textContent = label;
      detail.textContent = String(value ?? "");
      details.append(term, detail);
    }
    content.append(details);

    const contentHeading = document.createElement("h4");
    contentHeading.textContent = "Complete escaped source content";
    const contentPre = document.createElement("pre");
    contentPre.textContent = String(review.source_content_escaped ?? "");
    content.append(contentHeading, contentPre);

    const nonEffectsHeading = document.createElement("h4");
    nonEffectsHeading.textContent = "Explicit non-effects";
    const nonEffects = document.createElement("ul");
    for (const value of review.non_effects ?? []) {
      const item = document.createElement("li");
      item.textContent = String(value ?? "");
      nonEffects.append(item);
    }
    content.append(nonEffectsHeading, nonEffects);
    return content;
  }
  const details = document.createElement("dl");
  const values = [
    ["Operation", review.operation],
    ["Relative path", review.path],
    ["Byte range start", review.changedRange.start],
    ["Byte range end", review.changedRange.end],
    ["Byte range length", review.changedRange.length],
    ["BOM state", review.bom],
    ["Preimage EOF state", review.preimageEof],
    ["Preimage EOF detail", review.preimageEofMarker],
    ["Postimage EOF state", review.postimageEof],
    ["Postimage EOF detail", review.postimageEofMarker],
    ["Preimage SHA-256", review.preimageSha256],
    ["Preimage length", review.preimageByteLength],
    ["Postimage SHA-256", review.postimageSha256],
    ["Postimage length", review.postimageByteLength],
    ["Intended effect", review.intendedEffect],
    ["Unchanged context", review.unchangedContext],
  ];
  for (const [label, value] of values) {
    const term = document.createElement("dt");
    const detail = document.createElement("dd");
    term.textContent = label;
    detail.textContent = String(value ?? "");
    details.append(term, detail);
  }
  content.append(details);
  for (const [label, value] of [["Expected old text", review.oldTextEscaped], ["Replacement text", review.replacementTextEscaped]]) {
    const heading = document.createElement("strong");
    const pre = document.createElement("pre");
    heading.textContent = label;
    pre.textContent = String(value ?? "");
    content.append(heading, pre);
  }
  const nonEffects = document.createElement("ul");
  for (const value of review.nonEffects ?? []) {
    const item = document.createElement("li");
    item.textContent = value;
    nonEffects.append(item);
  }
  const nonEffectsHeading = document.createElement("strong");
  nonEffectsHeading.textContent = "Explicit non-effects";
  content.append(nonEffectsHeading, nonEffects);
  return content;
}

function clearActiveHostReview() {
  if (!activePreparedHostReview) return;
  activePreparedHostReview.actionStarted = true;
  if (activePreparedHostReview.dialog.open) activePreparedHostReview.dialog.close();
  activePreparedHostReview.dialog.remove();
  activePreparedHostReview = null;
}

function openHostReview(invoke, prepared, kind) {
  const confirmation = document.createElement("dialog");
  const title = document.createElement("h3");
  const confirm = document.createElement("button");
  const cancel = document.createElement("button");
  title.textContent = kind === "patch"
    ? "Review Host patch"
    : kind === "multi_file_edit" ? "Review Host multi-file edit"
      : kind === "create_file" ? "Review Host new-file creation"
        : kind === "delete_file" ? "Review Host file deletion"
          : kind === "rename_file" ? "Review Host file rename / move" : "Review Host action";
  confirm.type = "button";
  confirm.textContent = "Confirm Host action";
  cancel.type = "button";
  cancel.textContent = "Cancel";
  confirmation.append(title, renderHostReview(prepared.review, kind), confirm, cancel);
  const active = { dialog: confirmation, actionStarted: false, ticketId: prepared.ticketId };
  activePreparedHostReview = active;
  const settle = async (action) => {
    if (active.actionStarted) return;
    active.actionStarted = true;
    confirm.disabled = true;
    cancel.disabled = true;
    try {
      await invoke(action === "confirm" ? "host_confirm_tool_invocation" : "host_cancel_tool_invocation", { request: { ticketId: active.ticketId } });
    } catch (error) {
      showChatError(error);
    } finally {
      if (activePreparedHostReview === active) activePreparedHostReview = null;
      if (confirmation.open) confirmation.close();
      confirmation.remove();
    }
  };
  confirm.addEventListener("click", () => { void settle("confirm"); }, { once: true });
  cancel.addEventListener("click", () => { void settle("cancel"); }, { once: true });
  confirmation.addEventListener("cancel", (event) => {
    event.preventDefault();
    void settle("cancel");
  });
  confirmation.addEventListener("close", () => {
    if (!active.actionStarted) void settle("cancel");
  });
  document.body.append(confirmation);
  confirmation.showModal();
}

async function submitHostForm(invoke, form) {
  const kind = form.dataset.hostKind;
  const value = form.querySelector("[data-host-input]")?.value;
  if (kind === "repo_create_branch") {
    const prepared = await invoke("host_prepare_repo_create_branch", { request: { name: value } });
    openHostReview(invoke, prepared, "branch");
    return;
  }
  if (kind === "repo_patch") {
    const request = {
      path: form.querySelector('[data-host-input="path"]').value,
      expectedOldText: form.querySelector('[data-host-input="expectedOldText"]').value,
      replacementText: form.querySelector('[data-host-input="replacementText"]').value,
    };
    const prepared = await invoke("host_prepare_repo_patch", { request });
    openHostReview(invoke, prepared, "patch");
    return;
  }
  if (kind === "repo_edit_files") {
    const prepared = await invoke("host_prepare_repo_edit_files", { request: readMultiFileRequest(form) });
    openHostReview(invoke, prepared, "multi_file_edit");
    return;
  }
  const isCreateFile = kind === "repo_create_file";
  if (isCreateFile) {
    const request = {
      path: form.querySelector('[data-host-input="path"]').value,
      content: form.querySelector('[data-host-input="content"]').value,
    };
    const prepared = await invoke("host_prepare_repo_create_file", { request });
    openHostReview(invoke, prepared, "create_file");
    return;
  }
  if (kind === "repo_delete_file") {
    const request = {
      path: form.querySelector('[data-host-input="path"]').value,
    };
    const prepared = await invoke("host_prepare_repo_delete_file", { request });
    openHostReview(invoke, prepared, "delete_file");
    return;
  }
  if (kind === "repo_rename_file") {
    const request = {
      source_path: form.querySelector('[data-host-input="sourcePath"]').value,
      destination_path: form.querySelector('[data-host-input="destinationPath"]').value,
    };
    const prepared = await invoke("host_prepare_repo_rename_file", { request });
    openHostReview(invoke, prepared, "rename_file");
    return;
  }
  const request = { kind };
  if (value !== undefined) request.path = value;
  await invoke("host_invoke_read", { request });
}

function installHostFormHandlers(invoke) {
  const tools = document.querySelector("#effective-tools");
  tools.addEventListener("click", (event) => {
    const button = event.target.closest("button[data-multi-file-action]");
    if (!button) return;
    const form = button.closest("form[data-host-kind]");
    if (!form) return;
    const action = button.dataset.multiFileAction;
    if (action === "add-target") {
      const targets = form.querySelector("[data-multi-file-target]").parentElement;
      if (targets.children.length < multiFileMaxTargets) appendMultiFileTarget(targets);
    } else if (action === "remove-target") {
      const target = button.closest("[data-multi-file-target]");
      if (form.querySelectorAll("[data-multi-file-target]").length > 1) target.remove();
      updateMultiFileBoundControls(form);
    } else if (action === "add-replacement") {
      const replacements = button.closest("[data-multi-file-target]").querySelector("[data-multi-file-replacements]");
      if (replacements.children.length < multiFileMaxReplacements) appendMultiFileReplacement(replacements);
      updateMultiFileBoundControls(form);
    } else if (action === "remove-replacement") {
      const replacements = button.closest("[data-multi-file-replacements]");
      if (replacements.children.length > 1) button.closest("[data-multi-file-replacement]").remove();
      updateMultiFileBoundControls(form);
    } else if (action === "reset-draft") {
      resetMultiFileDraft(form);
    }
  });
  tools.addEventListener("submit", async (event) => {
    const form = event.target.closest("form[data-host-kind]");
    if (!form) return;
    event.preventDefault();
    const button = form.querySelector("button[type=submit]");
    button.disabled = true;
    try {
      await submitHostForm(invoke, form);
    } catch (error) {
      showChatError(error);
      button.disabled = false;
    }
  });
}

function renderUnavailableCapability(capability) {
  const item = document.createElement("li");
  item.className = "authority-entry";
  const title = document.createElement("strong");
  title.textContent = capability.publicToolName ?? "Known capability";
  const details = document.createElement("dl");
  details.append(renderAuthorityValue("State", authorityLabel("unavailableState", capability.state)), renderAuthorityValue("Reason", authorityLabel("unavailableReason", capability.reason)));
  item.append(title, details);
  return item;
}

async function refreshEffectiveAuthority(invoke) {
  const error = document.querySelector("#effective-authority-error");
  try {
    renderEffectiveAuthority(await invoke("get_effective_authority_snapshot"));
  } catch {
    renderEffectiveAuthority({ schemaVersion: 0 });
    error.textContent = "Effective authority snapshot unavailable";
    error.hidden = false;
  }
}

function renderTrustedProfileSelection(selection) {
  renderedTrustedProfileSelection = selection;
  document.querySelector("#trusted-profile-state").textContent = selection.selected
    ? "Configured — providers inactive"
    : (selection.remembered ? "Remembered — not restored" : "No profile remembered");
  document.querySelector("#trusted-profile-id").textContent = selection.profileId ?? "Not selected";
  document.querySelector("#trusted-profile-mcp-count").textContent = String(selection.mcpProviderCount ?? 0);
  document.querySelector("#trusted-profile-plugin-count").textContent = String(selection.processPluginCount ?? 0);
  document.querySelector("#trusted-profile-tool-count").textContent = String(selection.expectedToolCount ?? 0);
}

async function refreshTrustedProfileSelection(invoke) {
  renderTrustedProfileSelection(await invoke("trusted_profile_selection"));
}

function modelHint(provider) {
  const hints = {
    inherit: "Uses Codex host configuration",
    openai: "Codex built-in OpenAI provider",
    ollama: "Codex built-in Ollama provider",
    lm_studio: "Codex built-in LM Studio provider",
    llama_cpp: "",
  };
  return hints[provider] ?? "Invalid model configuration";
}

function renderModelConfiguration(configuration) {
  renderedModelConfiguration = configuration;
  const provider = document.querySelector("#model-provider");
  const model = document.querySelector("#model-identifier");
  provider.value = configuration.provider;
  model.value = configuration.model ?? "";
  model.disabled = configuration.provider === "inherit" || chatRunning;
  provider.disabled = chatRunning;
  const llama = configuration.provider === "llama_cpp";
  const endpointControls = document.querySelector("#llama-cpp-endpoint");
  endpointControls.hidden = !llama;
  for (const element of endpointControls.querySelectorAll("select, input")) element.disabled = chatRunning;
  if (configuration.endpoint) {
    document.querySelector("#llama-cpp-scheme").value = configuration.endpoint.scheme;
    document.querySelector("#llama-cpp-host").value = configuration.endpoint.host;
    document.querySelector("#llama-cpp-port").value = configuration.endpoint.port;
  }
  const normalized = document.querySelector("#llama-cpp-normalized-endpoint");
  normalized.textContent = configuration.endpoint?.normalized ?? "";
  normalized.hidden = !configuration.endpoint;
  document.querySelector("#llama-cpp-insecure-warning").hidden = !llama || !configuration.insecureTransport;
  const readiness = {
    not_tested: "Not tested", checking: "Checking…", ready: "Ready", loading: "Model loading",
    unreachable: "Unreachable", tls_failure: "TLS failure", check_failed: "Health check failed",
  };
  document.querySelector("#llama-cpp-readiness").textContent = readiness[configuration.readiness] ?? "Not tested";
  document.querySelector("#test-llama-cpp-endpoint").disabled = !llama || chatRunning || configuration.readiness === "checking";
  document.querySelector("#apply-model-configuration").disabled = chatRunning;
  document.querySelector("#reset-model-preferences").disabled = chatRunning;
  document.querySelector("#model-hint").textContent = modelHint(configuration.provider);
}

async function refreshModelConfiguration(invoke) {
  renderModelConfiguration(await invoke("model_configuration"));
}

function renderRepositorySnapshot(snapshot) {
  document.querySelector("#repository-path").textContent = snapshot.path;
  const status = document.querySelector("#repository-status-entries");
  const renderDiff = (element, files) => {
    element.replaceChildren(...(files.length ? files.map((file) => {
      const article = document.createElement("article");
      const title = document.createElement("strong");
      const patch = document.createElement("pre");
      title.textContent = `${file.changeKind} ${file.newPath ?? file.oldPath ?? "[unknown path]"} +${file.addedLines ?? 0} -${file.deletedLines ?? 0}`;
      patch.textContent = file.patch ?? (file.binary ? "Binary file changed" : "No patch");
      article.append(title, patch);
      if (file.unstageActionId) {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.repositoryAction = "unstage";
        button.dataset.actionId = file.unstageActionId;
        button.textContent = "Unstage";
        article.append(button);
      }
      return article;
    }) : [emptyEntry("No changes")]));
  };
  status.replaceChildren(...(snapshot.statusEntries.length ? snapshot.statusEntries.map((entry) => {
    const item = document.createElement("article");
    item.textContent = `${entry.indexState}/${entry.worktreeState} ${entry.path}`;
    if (entry.stageActionId) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.repositoryAction = "stage";
      button.dataset.actionId = entry.stageActionId;
      button.textContent = "Stage";
      item.append(" ", button);
    } else if (entry.stagingNote) {
      const note = document.createElement("span");
      note.textContent = ` — ${entry.stagingNote}`;
      item.append(note);
    }
    return item;
  }) : [emptyEntry("Working tree clean")]));
  renderDiff(document.querySelector("#worktree-diff-entries"), snapshot.worktreeDiff);
  renderDiff(document.querySelector("#staged-diff-entries"), snapshot.stagedDiff);
  renderedCommitReview = snapshot.review;
  const reviewState = ({
    no_staged_changes: "No staged changes",
    review_available: "Review available — complete within bounded observation",
    review_binary_unsupported: "Binary staged content — review is not authorizable in v0.12",
    review_unavailable: "Review unavailable",
  }[snapshot.review.state] ?? "Review unavailable");
  const authorization = snapshot.review.authorizationState;
  document.querySelector("#staged-review-state").textContent = authorization === "authorized_pending"
    ? "Authorized for one future commit request"
    : reviewState;
  const authorize = document.querySelector("#authorize-commit");
  authorize.hidden = !snapshot.review.canAuthorize;
  authorize.disabled = !snapshot.review.canAuthorize;
}

function emptyEntry(text) {
  const item = document.createElement("p");
  item.textContent = text;
  return item;
}

async function refreshRepository(invoke) {
  const error = document.querySelector("#repository-error");
  error.hidden = true;
  try {
    renderRepositorySnapshot(await invoke("repository_snapshot"));
  } catch (repositoryError) {
    error.textContent = errorMessage(repositoryError);
    error.hidden = false;
  }
}

function appendActivity(payload) {
  const entries = document.querySelector("#activity-entries");
  const entry = document.createElement("article");
  const tool = document.createElement("strong");
  const state = document.createElement("span");
  const labels = {
    tool_requested: "Requested",
    tool_started: "Running",
    tool_finished: payload.result === "failed" ? "Failed" : "Completed",
  };

  entry.className = "activity-entry";
  entry.dataset.state = payload.kind === "tool_finished" ? payload.result : payload.kind;
  tool.textContent = payload.tool;
  state.textContent = labels[payload.kind] ?? "Unknown";
  entry.append(tool, state);
  if (payload.commit) {
    const commit = document.createElement("span");
    const commitLabels = {
      invalid_input: "Commit input rejected",
      precondition_failed: "Commit not performed — review again",
      known_no_effect: "Commit had no effect — review again",
      committed_verified: "Commit verified",
      uncertain: "Commit outcome uncertain — inspect manually; do not retry",
    };
    commit.textContent = commitLabels[payload.commit.status] ?? "Commit result unavailable";
    entry.append(commit);
    if (payload.commit.commitOid) {
      const oid = document.createElement("code");
      oid.textContent = payload.commit.commitOid;
      entry.append(oid);
    }
  }
  entries.append(entry);
  while (entries.children.length > maxActivityEntries) {
    entries.firstElementChild.remove();
  }
  entries.scrollTop = entries.scrollHeight;
}

function showBackendError() {
  const error = document.querySelector("#backend-error");
  error.textContent = "Desktop backend unavailable";
  error.hidden = false;
  document.querySelector("#codex-connection").disabled = true;
}

function waitForTauriApi() {
  return new Promise((resolve) => {
    let attempts = 0;
    const check = () => {
      const tauri = window.__TAURI__;
      if (tauri?.core?.invoke && tauri?.event?.listen) {
        resolve(tauri);
        return;
      }
      attempts += 1;
      if (attempts >= tauriApiRetryAttempts) {
        resolve(null);
        return;
      }
      window.setTimeout(check, tauriApiRetryDelayMs);
    };
    check();
  });
}

async function loadStatus(invoke) {
  const status = await invoke("app_status");
  renderRows(document.querySelector("#application-status"), applicationRows, status);
  renderRows(document.querySelector("#runtime-status"), runtimeRows, status);
  const button = document.querySelector("#codex-connection");
  const connectionError = document.querySelector("#connection-error");
  button.disabled = status.codexStatus === "connecting" || status.codexStatus === "disconnecting" || chatRunning;
  button.textContent = status.codexStatus === "connected" ? "Disconnect Codex" : "Connect Codex";
  const connected = status.codexStatus === "connected";
  const reconnectRequired = status.repositoryToolsStatus === "reconnect required"
    || status.modelConfigurationStatus === "reconnect required"
    || status.profileStatus === "reconnect required";
  const prompt = document.querySelector("#chat-prompt");
  const send = document.querySelector("#chat-send");
  document.querySelector("#chat-hint").textContent = connected
    ? (reconnectRequired ? "Reconnect Codex to chat" : (chatRunning ? "Chat running" : "Chat ready"))
    : "Connect Codex to chat";
  prompt.disabled = !connected || reconnectRequired || chatRunning;
  send.disabled = !connected || reconnectRequired;
  send.textContent = chatRunning ? "Cancel Turn" : "Send";
  document.querySelector("#new-conversation").disabled = chatRunning;
  document.querySelector("#resume-previous-conversation").disabled = !resumeAvailable
    || !connected
    || chatRunning
    || status.repositoryToolsStatus === "reconnect required"
    || status.modelConfigurationStatus === "reconnect required"
    || status.profileStatus === "reconnect required"
    || resumeUsed;
  document.querySelector("#clear-conversation-history").disabled = chatRunning;
  document.querySelector("#choose-repository").disabled = status.codexStatus === "connecting" || status.codexStatus === "disconnecting" || chatRunning;
  const profileSelectionAllowed = ["not connected", "error"].includes(status.codexStatus) && !chatRunning;
  document.querySelector("#choose-trusted-profile").disabled = !profileSelectionAllowed;
  document.querySelector("#restore-trusted-profile").disabled = !profileSelectionAllowed || !renderedTrustedProfileSelection?.remembered;
  const profileForgetAllowed = ["not connected", "error", "connected"].includes(status.codexStatus) && !chatRunning;
  document.querySelector("#forget-trusted-profile").disabled = !profileForgetAllowed || !renderedTrustedProfileSelection?.remembered;
  document.querySelector("#clear-trusted-profile").disabled = !profileSelectionAllowed || !renderedTrustedProfileSelection?.selected;
  const model = document.querySelector("#model-identifier");
  const provider = document.querySelector("#model-provider");
  const endpointControls = document.querySelector("#llama-cpp-endpoint");
  document.querySelector("#apply-model-configuration").disabled = chatRunning;
  document.querySelector("#reset-model-preferences").disabled = chatRunning;
  provider.disabled = chatRunning;
  model.disabled = chatRunning || provider.value === "inherit";
  for (const element of endpointControls.querySelectorAll("select, input")) {
    element.disabled = chatRunning;
  }
  if (status.codexError) {
    connectionError.textContent = errorMessage(status.codexError);
    connectionError.hidden = false;
  } else {
    connectionError.hidden = true;
  }
}

function appendMessage(role, text) {
  const messages = document.querySelector("#chat-messages");
  const message = document.createElement("article");
  const label = document.createElement("strong");
  const content = document.createElement("p");
  message.className = "chat-message";
  label.textContent = role;
  content.textContent = text;
  message.append(label, content);
  messages.append(message);
  messages.scrollTop = messages.scrollHeight;
  return content;
}

function appendContextSeparator(reason) {
  const messages = document.querySelector("#chat-messages");
  const separator = document.createElement("section");
  const title = document.createElement("strong");
  const detail = document.createElement("p");
  const labels = {
    new_conversation: "New conversation context",
    repository_changed: "Repository changed",
    model_configuration_changed: "Model configuration changed",
    repository_and_model_changed: "Repository and model configuration changed",
    application_restarted: "Application restarted",
    history_trimmed: "Earlier conversation history was trimmed",
  };
  separator.className = "conversation-context-separator";
  title.textContent = reason === "history_trimmed" ? "Conversation history" : "New conversation context";
  detail.textContent = labels[reason] ?? "New conversation context";
  separator.append(title, detail);
  messages.append(separator);
  messages.scrollTop = messages.scrollHeight;
}

function showPersistenceWarning(warning) {
  const messages = {
    restore_failed: "Previous conversation could not be restored.",
    save_failed: "Conversation history could not be saved.",
  };
  const error = document.querySelector("#chat-error");
  error.textContent = messages[warning] ?? "Conversation history could not be saved.";
  error.hidden = false;
}

function renderTranscript(transcript) {
  resumeAvailable = transcript.resumeAvailable;
  for (const record of transcript.records) {
    if (record.kind === "completed_message") {
      appendMessage(record.role === "user" ? "You" : "RAH", record.text);
    } else if (record.kind === "context_separator") {
      appendContextSeparator(record.reason);
    }
  }
  if (transcript.warning) showPersistenceWarning(transcript.warning);
}

async function replaceTranscript(invoke) {
  // A repository namespace is a presentation boundary, not a separator in a
  // shared transcript. Remove every rendered record before displaying only the
  // newly selected namespace.
  document.querySelector("#chat-messages").replaceChildren();
  activeAssistant = null;
  resumeUsed = false;
  renderTranscript(await invoke("conversation_transcript"));
}

function showChatError(code) {
  const error = document.querySelector("#chat-error");
  error.textContent = errorMessage(code);
  error.hidden = false;
}

function handleChatEvent(invoke, event) {
  const payload = event.payload;
  if (payload.kind === "started") {
    activeAssistant = appendMessage("RAH", "");
  } else if (payload.kind === "delta" && activeAssistant) {
    activeAssistant.textContent += payload.text;
  } else if (payload.kind === "failed" || payload.kind === "cancelled") {
    showChatError(payload.code);
  }
  if (["completed", "failed", "cancelled"].includes(payload.kind)) {
    chatRunning = false;
    activeAssistant = null;
    void loadStatus(invoke).catch(() => showBackendError());
  }
}

async function toggleCodexConnection(invoke) {
  const button = document.querySelector("#codex-connection");
  const error = document.querySelector("#connection-error");
  button.disabled = true;
  error.hidden = true;
  let disconnected = false;
  try {
    const status = await invoke("app_status");
    disconnected = status.codexStatus === "connected";
    await invoke(disconnected ? "disconnect_codex" : "connect_codex");
  } catch (connectionError) {
    error.textContent = errorMessage(connectionError);
    error.hidden = false;
  } finally {
    try {
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
      if (disconnected) await refreshRepository(invoke);
    } catch (statusError) {
      console.error("failed to refresh desktop status", statusError);
      showBackendError();
    }
  }
}

async function initializeDesktop() {
  const tauri = await waitForTauriApi();
  if (!tauri) {
    throw new Error("supported Tauri global API is unavailable");
  }
  const { invoke } = tauri.core;
  const { listen } = tauri.event;

  await listen("chat_event", (event) => handleChatEvent(invoke, event));
  await listen("activity_event", (event) => appendActivity(event.payload));
  await listen("host_activity_event", (event) => {
    appendHostActivity(event.payload);
    void refreshEffectiveAuthority(invoke);
    void loadStatus(invoke).catch(() => showBackendError());
  });
  await listen("conversation_persistence_warning", (event) => showPersistenceWarning(event.payload));
  await listen("desktop_preferences_warning", (event) => {
    const error = document.querySelector("#model-error");
    error.textContent = event.payload === "preferences_restore_failed" ? "Model preferences could not be restored." : "Model preferences could not be saved.";
    error.hidden = false;
  });
  await listen("repository_snapshot_refresh", () => {
    void refreshRepository(invoke);
    void refreshEffectiveAuthority(invoke);
  });
  document.querySelector("#refresh-effective-authority").addEventListener("click", () => {
    void refreshEffectiveAuthority(invoke);
  });
  installHostFormHandlers(invoke);
  document.querySelector("#codex-connection").addEventListener("click", () => {
    void toggleCodexConnection(invoke);
  });
  document.querySelector("#choose-trusted-profile").addEventListener("click", async () => {
    const error = document.querySelector("#trusted-profile-error");
    error.hidden = true;
    try {
      await invoke("choose_trusted_profile");
      await refreshTrustedProfileSelection(invoke);
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (profileError) {
      error.textContent = errorMessage(profileError);
      error.hidden = false;
    }
  });
  document.querySelector("#restore-trusted-profile").addEventListener("click", async () => {
    const error = document.querySelector("#trusted-profile-error");
    error.hidden = true;
    try {
      await invoke("restore_trusted_profile");
      await refreshTrustedProfileSelection(invoke);
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (profileError) {
      error.textContent = errorMessage(profileError);
      error.hidden = false;
    }
  });
  document.querySelector("#forget-trusted-profile").addEventListener("click", async () => {
    const error = document.querySelector("#trusted-profile-error");
    error.hidden = true;
    try {
      await invoke("forget_trusted_profile");
      await refreshTrustedProfileSelection(invoke);
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (profileError) {
      error.textContent = errorMessage(profileError);
      error.hidden = false;
    }
  });
  document.querySelector("#clear-trusted-profile").addEventListener("click", async () => {
    const error = document.querySelector("#trusted-profile-error");
    error.hidden = true;
    try {
      await invoke("clear_trusted_profile");
      await refreshTrustedProfileSelection(invoke);
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (profileError) {
      error.textContent = errorMessage(profileError);
      error.hidden = false;
    }
  });
  document.querySelector("#choose-repository").addEventListener("click", async () => {
    const error = document.querySelector("#repository-error");
    error.hidden = true;
    try {
      await invoke("choose_repository");
      await replaceTranscript(invoke);
      await loadStatus(invoke);
      await refreshRepository(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (repositoryError) {
      error.textContent = errorMessage(repositoryError);
      error.hidden = false;
    }
  });
  document.querySelector("#refresh-repository").addEventListener("click", () => {
    void refreshRepository(invoke);
    void refreshEffectiveAuthority(invoke);
  });
  document.querySelector("#new-conversation").addEventListener("click", async () => {
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      await invoke("new_conversation");
      resumeUsed = true;
      appendContextSeparator();
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  document.querySelector("#clear-conversation-history").addEventListener("click", async () => {
    document.querySelector("#clear-history-confirmation").showModal();
  });
  document.querySelector("#clear-history-confirmation").addEventListener("close", async (event) => {
    if (event.target.returnValue !== "confirm") return;
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      await invoke("clear_conversation_history");
      resumeAvailable = false;
      resumeUsed = true;
      document.querySelector("#chat-messages").replaceChildren();
      appendMessage("RAH", "Conversation history cleared");
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  document.querySelector("#save-commit-identity").addEventListener("click", async () => {
    try {
      await invoke("set_commit_identity", {
        name: document.querySelector("#commit-identity-name").value,
        email: document.querySelector("#commit-identity-email").value,
      });
      const identity = await invoke("commit_identity");
      document.querySelector("#commit-identity-state").textContent = identity.configured ? "Configured" : "Not configured";
      await refreshRepository(invoke);
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (error) {
      document.querySelector("#repository-error").textContent = errorMessage(error);
      document.querySelector("#repository-error").hidden = false;
    }
  });
  document.querySelector("#authorize-commit").addEventListener("click", async () => {
    if (!renderedCommitReview?.reviewId) return;
    try {
      const result = await invoke("repository_authorize_commit_review", { reviewId: renderedCommitReview.reviewId });
      document.querySelector("#staged-review-state").textContent = result.authorizationState === "authorized_pending"
        ? "Authorized for one future commit request" : "Review authorization unavailable";
      document.querySelector("#authorize-commit").hidden = true;
      await refreshEffectiveAuthority(invoke);
    } catch (error) {
      document.querySelector("#repository-error").textContent = errorMessage(error);
      document.querySelector("#repository-error").hidden = false;
    }
  });
  document.querySelector(".repository").addEventListener("click", async (event) => {
    const button = event.target.closest("button[data-repository-action]");
    if (!button) return;
    const error = document.querySelector("#repository-error");
    error.hidden = true;
    button.disabled = true;
    try {
      await invoke(button.dataset.repositoryAction === "stage" ? "repository_stage_action" : "repository_unstage_action", { actionId: button.dataset.actionId });
      await refreshRepository(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (repositoryError) {
      error.textContent = errorMessage(repositoryError);
      error.hidden = false;
      await refreshRepository(invoke);
    }
  });
  document.querySelector("#resume-previous-conversation").addEventListener("click", async () => {
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      await invoke("resume_previous_conversation");
      resumeUsed = true;
      const success = document.querySelector("#resume-success");
      success.textContent = "Previous conversation resumed in current context.";
      success.hidden = false;
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  document.querySelector("#model-provider").addEventListener("change", () => {
    const provider = document.querySelector("#model-provider");
    const model = document.querySelector("#model-identifier");
    model.disabled = chatRunning || provider.value === "inherit";
    document.querySelector("#llama-cpp-endpoint").hidden = provider.value !== "llama_cpp";
    document.querySelector("#model-hint").textContent = modelHint(provider.value);
  });
  document.querySelector("#apply-model-configuration").addEventListener("click", async () => {
    const error = document.querySelector("#model-error");
    error.hidden = true;
    const provider = document.querySelector("#model-provider").value;
    const model = document.querySelector("#model-identifier").value;
    const llamaCppEndpoint = provider === "llama_cpp" ? {
      scheme: document.querySelector("#llama-cpp-scheme").value,
      host: document.querySelector("#llama-cpp-host").value,
      port: Number(document.querySelector("#llama-cpp-port").value),
    } : null;
    try {
      await invoke("set_model_configuration", {
        provider,
        model: provider === "inherit" ? null : model,
        llamaCppEndpoint,
      });
      await refreshModelConfiguration(invoke);
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
      if ((await invoke("model_configuration")).status === "reconnect required") {
        error.textContent = "Reconnect Codex to activate this model configuration.";
        error.hidden = false;
      }
    } catch (modelError) {
      error.textContent = errorMessage(modelError);
      error.hidden = false;
    }
  });
  document.querySelector("#reset-model-preferences").addEventListener("click", async () => {
    const error = document.querySelector("#model-error");
    error.hidden = true;
    try {
      await invoke("reset_model_preferences");
      await refreshModelConfiguration(invoke);
      await loadStatus(invoke);
      await refreshEffectiveAuthority(invoke);
    } catch (resetError) {
      error.textContent = errorMessage(resetError);
      error.hidden = false;
    }
  });
  document.querySelector("#test-llama-cpp-endpoint").addEventListener("click", async () => {
    const error = document.querySelector("#model-error");
    error.hidden = true;
    try {
      const readinessRequest = invoke("test_llama_cpp_endpoint");
      if (renderedModelConfiguration) {
        renderModelConfiguration({ ...renderedModelConfiguration, readiness: "checking" });
      }
      await readinessRequest;
      const refreshReadiness = async () => {
        const configuration = await invoke("model_configuration");
        renderModelConfiguration(configuration);
        if (configuration.readiness === "checking") {
          window.setTimeout(() => { void refreshReadiness(); }, 100);
        }
      };
      void refreshReadiness();
    } catch (readinessError) {
      if (renderedModelConfiguration) {
        renderModelConfiguration({ ...renderedModelConfiguration, readiness: "check_failed" });
      }
      error.textContent = "Health check failed";
      error.hidden = false;
    }
  });
  document.querySelector("#chat-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    if (chatRunning) {
      try {
        await invoke("cancel_chat");
      } catch (error) {
        showChatError(error);
      }
      return;
    }
    const prompt = document.querySelector("#chat-prompt");
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      const result = await invoke("send_chat", { prompt: prompt.value });
      if (result.contextChange) appendContextSeparator(result.contextChange);
      appendMessage("You", prompt.value);
      prompt.value = "";
      chatRunning = true;
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  await refreshTrustedProfileSelection(invoke);
  await loadStatus(invoke);
  await replaceTranscript(invoke);
  await refreshModelConfiguration(invoke);
  await refreshEffectiveAuthority(invoke);
  const identity = await invoke("commit_identity");
  document.querySelector("#commit-identity-state").textContent = identity.configured ? "Configured" : "Not configured";
  const preferencesWarning = await invoke("desktop_preferences_warning");
  if (preferencesWarning) {
    const error = document.querySelector("#model-error");
    error.textContent = preferencesWarning === "preferences_restore_failed" ? "Model preferences could not be restored." : "Model preferences could not be saved.";
    error.hidden = false;
  }
  setFrontendBootStatus("Desktop UI ready");
}

function startDesktop() {
  void initializeDesktop().catch((error) => {
    console.error("failed to initialize desktop frontend", error);
    setFrontendBootStatus(
      error.message === "supported Tauri global API is unavailable"
        ? "Desktop frontend unavailable"
        : "Desktop backend unavailable",
    );
    showBackendError();
  });
}

if (document.readyState === "complete") {
  startDesktop();
} else {
  window.addEventListener("load", startDesktop, { once: true });
}
