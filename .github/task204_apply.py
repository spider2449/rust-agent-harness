from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"expected exactly one match in {path}, found {count}: {old[:100]!r}"
        )
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


module = r'''#![cfg(target_os = "windows")]

use std::path::PathBuf;

use rah_tools::TrustedStaticProfile;
use serde::Serialize;

/// Desktop-owned configured intent for one explicitly selected Trusted Profile.
///
/// The source path remains Rust-only. Task 204 never turns this value into an
/// effective provider composition and never publishes the source through IPC.
#[derive(Clone, Debug)]
pub(crate) struct DesktopTrustedProfileSelection {
    source: PathBuf,
    profile_id: String,
    mcp_provider_count: u32,
    process_plugin_count: u32,
    expected_tool_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProfileSelectionError {
    InvalidProfile,
    FirstPartyCapabilities,
}

/// Bounded, sanitized configured state exposed to the Desktop frontend.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrustedProfilePresentation {
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub mcp_provider_count: u32,
    pub process_plugin_count: u32,
    pub configured_provider_count: u32,
    pub expected_tool_count: u32,
}

impl TrustedProfilePresentation {
    #[must_use]
    pub(crate) fn none() -> Self {
        Self {
            selected: false,
            profile_id: None,
            mcp_provider_count: 0,
            process_plugin_count: 0,
            configured_provider_count: 0,
            expected_tool_count: 0,
        }
    }
}

impl DesktopTrustedProfileSelection {
    #[must_use]
    pub(crate) fn presentation(&self) -> TrustedProfilePresentation {
        // Read the host-only source solely to preserve the invariant locally;
        // no path-derived value crosses the IPC boundary.
        debug_assert!(self.source.is_absolute());
        TrustedProfilePresentation {
            selected: true,
            profile_id: Some(self.profile_id.clone()),
            mcp_provider_count: self.mcp_provider_count,
            process_plugin_count: self.process_plugin_count,
            configured_provider_count: self
                .mcp_provider_count
                .saturating_add(self.process_plugin_count),
            expected_tool_count: self.expected_tool_count,
        }
    }
}

/// Performs hardened static loading plus the Desktop provider-only overlay rule.
///
/// `TrustedStaticProfile::load` is intentionally non-spawning. The returned
/// selection retains only configured intent; the loaded profile is dropped.
pub(crate) fn load_provider_only_profile(
    source: PathBuf,
) -> Result<DesktopTrustedProfileSelection, ProfileSelectionError> {
    let profile = TrustedStaticProfile::load(&source)
        .map_err(|_| ProfileSelectionError::InvalidProfile)?;
    if !profile.effective_profile().capabilities.is_empty() {
        return Err(ProfileSelectionError::FirstPartyCapabilities);
    }

    let mcp_provider_count = u32::try_from(profile.mcp_providers().len())
        .map_err(|_| ProfileSelectionError::InvalidProfile)?;
    let process_plugin_count = u32::try_from(profile.process_plugins().len())
        .map_err(|_| ProfileSelectionError::InvalidProfile)?;
    let expected_tool_count = profile
        .mcp_providers()
        .iter()
        .map(|provider| provider.tools().len())
        .chain(
            profile
                .process_plugins()
                .iter()
                .map(|provider| provider.tools().len()),
        )
        .try_fold(0usize, |total, count| total.checked_add(count))
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(ProfileSelectionError::InvalidProfile)?;

    Ok(DesktopTrustedProfileSelection {
        source,
        profile_id: profile.effective_profile().profile_id.clone(),
        mcp_provider_count,
        process_plugin_count,
        expected_tool_count,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde_json::json;

    use super::{
        ProfileSelectionError, TrustedProfilePresentation, load_provider_only_profile,
    };

    static NEXT_PROFILE: AtomicU64 = AtomicU64::new(1);

    struct ProfileFile(PathBuf);

    impl ProfileFile {
        fn write(value: serde_json::Value) -> Self {
            let path = std::env::temp_dir().join(format!(
                "rah-task204-profile-{}-{}.json",
                std::process::id(),
                NEXT_PROFILE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::write(
                &path,
                serde_json::to_vec(&value).expect("profile JSON should encode"),
            )
            .expect("profile fixture should be written");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ProfileFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn base_profile() -> serde_json::Value {
        json!({
            "profile_version": 1,
            "profile_id": "desktop-provider-overlay",
            "resources": {
                "executables": {},
                "repositories": {}
            },
            "capabilities": [],
            "mcp_providers": [],
            "process_plugins": []
        })
    }

    #[test]
    fn empty_provider_only_profile_is_configured_without_source_disclosure() {
        let fixture = ProfileFile::write(base_profile());
        let selection = load_provider_only_profile(fixture.path().to_path_buf())
            .expect("provider-only profile should be selected");
        let presentation = selection.presentation();
        assert_eq!(
            presentation,
            TrustedProfilePresentation {
                selected: true,
                profile_id: Some("desktop-provider-overlay".to_owned()),
                mcp_provider_count: 0,
                process_plugin_count: 0,
                configured_provider_count: 0,
                expected_tool_count: 0,
            }
        );
        let encoded = serde_json::to_string(&presentation).expect("presentation should encode");
        assert!(!encoded.contains(&fixture.path().display().to_string()));
    }

    #[test]
    fn first_party_capability_declaration_fails_closed_even_when_disabled() {
        let mut value = base_profile();
        value["capabilities"] = json!([{
            "name": "fs.read",
            "enabled": false,
            "permission": "read"
        }]);
        let fixture = ProfileFile::write(value);
        let error = load_provider_only_profile(fixture.path().to_path_buf())
            .expect_err("Desktop must reject every first-party capability declaration");
        assert_eq!(error, ProfileSelectionError::FirstPartyCapabilities);
    }

    #[test]
    fn external_provider_is_not_spawned_or_required_to_exist_during_selection() {
        let mut value = base_profile();
        value["resources"]["executables"] = json!({
            "missing-provider": {
                "path": "C:\\rah-task204-provider-does-not-exist.exe",
                "kind": "native"
            }
        });
        value["mcp_providers"] = json!([{
            "id": "fixture",
            "executable": "missing-provider",
            "tools": [{
                "remote_name": "echo",
                "permission": "none",
                "input_schema": {"type": "object"}
            }]
        }]);
        let fixture = ProfileFile::write(value);
        let presentation = load_provider_only_profile(fixture.path().to_path_buf())
            .expect("static selection must not require provider startup")
            .presentation();
        assert_eq!(presentation.mcp_provider_count, 1);
        assert_eq!(presentation.expected_tool_count, 1);
    }

    #[test]
    fn invalid_profile_is_collapsed_to_bounded_selection_error() {
        let fixture = ProfileFile::write(json!({"profile_version": 1}));
        let error = load_provider_only_profile(fixture.path().to_path_buf())
            .expect_err("invalid profile must fail closed");
        assert_eq!(error, ProfileSelectionError::InvalidProfile);
    }
}
'''
Path("crates/rah-desktop/src/trusted_profile_selection.rs").write_text(
    module, encoding="utf-8"
)

replace_once(
    "crates/rah-desktop/src/main.rs",
    '#[cfg(target_os = "windows")]\nmod git_discovery;\n',
    '#[cfg(target_os = "windows")]\nmod git_discovery;\n#[cfg(target_os = "windows")]\nmod trusted_profile_selection;\n',
)
replace_once(
    "crates/rah-desktop/src/main.rs",
    'use effective_authority::{\n    ConfiguredSummary, ConnectionBinding, ConnectionBindingState, DesktopToolComposition,\n    EffectiveAuthoritySnapshot, RepositoryBinding, RepositoryIdentity, RepositoryKind,\n    SnapshotStatus, SourceKind,\n};\n',
    'use effective_authority::{\n    ConfiguredSummary, ConnectionBinding, ConnectionBindingState, DesktopToolComposition,\n    EffectiveAuthoritySnapshot, RepositoryBinding, RepositoryIdentity, RepositoryKind,\n    SnapshotStatus, SourceKind,\n};\n#[cfg(target_os = "windows")]\nuse trusted_profile_selection::{\n    DesktopTrustedProfileSelection, ProfileSelectionError, TrustedProfilePresentation,\n    load_provider_only_profile,\n};\n',
)
replace_once(
    "crates/rah-desktop/src/main.rs",
    '    commit_capability: Mutex<Option<DesktopCommitCapability>>,\n    /// An app-owned non-project directory used only when no repository is selected.\n',
    '    commit_capability: Mutex<Option<DesktopCommitCapability>>,\n    /// Explicit host-selected Trusted Profile intent. Static selection never activates providers.\n    trusted_profile: Mutex<Option<DesktopTrustedProfileSelection>>,\n    /// An app-owned non-project directory used only when no repository is selected.\n',
)
replace_once(
    "crates/rah-desktop/src/main.rs",
    '            commit_capability: Mutex::new(None),\n            neutral_workspace,\n',
    '            commit_capability: Mutex::new(None),\n            trusted_profile: Mutex::new(None),\n            neutral_workspace,\n',
)
replace_once(
    "crates/rah-desktop/src/main.rs",
    '    ToolRegistryFailed,\n    ChatEmptyPrompt,\n',
    '    ToolRegistryFailed,\n    ProfileInvalid,\n    ProfileFirstPartyCapabilitiesUnsupported,\n    ProfileDialogFailed,\n    ProfileBusy,\n    ChatEmptyPrompt,\n',
)
replace_once(
    "crates/rah-desktop/src/main.rs",
    '''        let model_generation = self
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation;
        current_app_status(
            &connection,
            repository_selected,
            repository_generation,
            model_generation,
        )
''',
    '''        let model_generation = self
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation;
        let profile_selected = self
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let mut status = current_app_status(
            &connection,
            repository_selected,
            repository_generation,
            model_generation,
        );
        status.profile_status = if profile_selected {
            "configured; providers inactive"
        } else {
            "not loaded"
        };
        status
''',
)
replace_once(
    "crates/rah-desktop/src/main.rs",
    '''#[cfg(target_os = "windows")]
#[tauri::command]
fn model_configuration(state: State<'_, DesktopAppState>) -> ModelConfigurationPresentation {
''',
    '''#[cfg(target_os = "windows")]
fn trusted_profile_selection_allowed(
    chat: ChatState,
    connection: &ConnectionState,
) -> Result<(), FrontendError> {
    if chat != ChatState::Idle
        || !matches!(connection, ConnectionState::NotConnected | ConnectionState::Error(_))
    {
        Err(FrontendError::ProfileBusy)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn ensure_trusted_profile_selection_allowed(
    state: &DesktopAppState,
) -> Result<(), FrontendError> {
    let chat = *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    trusted_profile_selection_allowed(chat, &connection)
}

#[cfg(target_os = "windows")]
fn profile_selection_error(error: ProfileSelectionError) -> FrontendError {
    match error {
        ProfileSelectionError::InvalidProfile => FrontendError::ProfileInvalid,
        ProfileSelectionError::FirstPartyCapabilities => {
            FrontendError::ProfileFirstPartyCapabilitiesUnsupported
        }
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn trusted_profile_selection(
    state: State<'_, DesktopAppState>,
) -> TrustedProfilePresentation {
    state
        .trusted_profile
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or_else(TrustedProfilePresentation::none, |profile| profile.presentation())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn choose_trusted_profile(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    ensure_trusted_profile_selection_allowed(state.inner())?;
    let selected = app.dialog().file().blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(());
    };
    let path = selected
        .into_path()
        .map_err(|_| FrontendError::ProfileDialogFailed)?;
    // A connection may have started while the native picker was open.
    ensure_trusted_profile_selection_allowed(state.inner())?;
    let selection = load_provider_only_profile(path).map_err(|error| {
        tracing::warn!(reason = ?error, "Desktop Trusted Profile static validation failed");
        profile_selection_error(error)
    })?;
    // Static loading is intentionally not an activation lock. Revalidate host
    // lifecycle immediately before publishing configured intent.
    ensure_trusted_profile_selection_allowed(state.inner())?;
    *state
        .trusted_profile
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(selection);
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn clear_trusted_profile(state: State<'_, DesktopAppState>) -> Result<(), FrontendError> {
    ensure_trusted_profile_selection_allowed(state.inner())?;
    *state
        .trusted_profile
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn model_configuration(state: State<'_, DesktopAppState>) -> ModelConfigurationPresentation {
''',
)
replace_once(
    "crates/rah-desktop/src/main.rs",
    '''            app_status,
            model_configuration,
''',
    '''            app_status,
            trusted_profile_selection,
            choose_trusted_profile,
            clear_trusted_profile,
            model_configuration,
''',
)

replace_once(
    "crates/rah-desktop/build.rs",
    '''            "app_status",
            "model_configuration",
''',
    '''            "app_status",
            "trusted_profile_selection",
            "choose_trusted_profile",
            "clear_trusted_profile",
            "model_configuration",
''',
)

replace_once(
    "crates/rah-desktop/capabilities/default.json",
    '''    "allow-app-status",
    "allow-model-configuration",
''',
    '''    "allow-app-status",
    "allow-trusted-profile-selection",
    "allow-choose-trusted-profile",
    "allow-clear-trusted-profile",
    "allow-model-configuration",
''',
)

replace_once(
    "crates/rah-desktop/frontend/index.html",
    '''        <section aria-labelledby="runtime-status-title">
''',
    '''        <section class="profile" aria-labelledby="trusted-profile-title">
          <h2 id="trusted-profile-title">Trusted Profile</h2>
          <p id="trusted-profile-error" class="connection-error" hidden></p>
          <p id="trusted-profile-state">Not selected</p>
          <dl>
            <div><dt>Profile ID</dt><dd id="trusted-profile-id">Not selected</dd></div>
            <div><dt>MCP providers</dt><dd id="trusted-profile-mcp-count">0</dd></div>
            <div><dt>Process Plugins</dt><dd id="trusted-profile-plugin-count">0</dd></div>
            <div><dt>Expected external Tools</dt><dd id="trusted-profile-tool-count">0</dd></div>
          </dl>
          <div class="repository-actions">
            <button id="choose-trusted-profile" type="button">Choose Profile</button>
            <button id="clear-trusted-profile" type="button" disabled>Clear Profile</button>
          </div>
          <p class="model-hint">Configured intent only in this build. Static validation does not start, advertise, or connect MCP or Process Plugin providers.</p>
        </section>
        <section aria-labelledby="runtime-status-title">
''',
)

replace_once(
    "crates/rah-desktop/frontend/status.js",
    '''let renderedModelConfiguration = null;
let renderedCommitReview = null;
''',
    '''let renderedModelConfiguration = null;
let renderedCommitReview = null;
let renderedTrustedProfileSelection = null;
''',
)
replace_once(
    "crates/rah-desktop/frontend/status.js",
    '''    tool_registry_failed: "Desktop tool registry unavailable",
    chat_empty_prompt: "Enter a message before sending",
''',
    '''    tool_registry_failed: "Desktop tool registry unavailable",
    profile_invalid: "Trusted Profile is invalid or unsupported",
    profile_first_party_capabilities_unsupported: "Desktop v0.17 accepts provider-only Trusted Profiles; remove first-party capabilities",
    profile_dialog_failed: "Trusted Profile picker failed",
    profile_busy: "Trusted Profile selection is available only while Codex is disconnected",
    chat_empty_prompt: "Enter a message before sending",
''',
)
replace_once(
    "crates/rah-desktop/frontend/status.js",
    '''function modelHint(provider) {
''',
    '''function renderTrustedProfileSelection(selection) {
  renderedTrustedProfileSelection = selection;
  document.querySelector("#trusted-profile-state").textContent = selection.selected
    ? "Configured — providers inactive"
    : "Not selected";
  document.querySelector("#trusted-profile-id").textContent = selection.profileId ?? "Not selected";
  document.querySelector("#trusted-profile-mcp-count").textContent = String(selection.mcpProviderCount ?? 0);
  document.querySelector("#trusted-profile-plugin-count").textContent = String(selection.processPluginCount ?? 0);
  document.querySelector("#trusted-profile-tool-count").textContent = String(selection.expectedToolCount ?? 0);
}

async function refreshTrustedProfileSelection(invoke) {
  renderTrustedProfileSelection(await invoke("trusted_profile_selection"));
}

function modelHint(provider) {
''',
)
replace_once(
    "crates/rah-desktop/frontend/status.js",
    '''  document.querySelector("#choose-repository").disabled = status.codexStatus === "connecting" || status.codexStatus === "disconnecting" || chatRunning;
  const model = document.querySelector("#model-identifier");
''',
    '''  document.querySelector("#choose-repository").disabled = status.codexStatus === "connecting" || status.codexStatus === "disconnecting" || chatRunning;
  const profileSelectionAllowed = ["not connected", "error"].includes(status.codexStatus) && !chatRunning;
  document.querySelector("#choose-trusted-profile").disabled = !profileSelectionAllowed;
  document.querySelector("#clear-trusted-profile").disabled = !profileSelectionAllowed || !renderedTrustedProfileSelection?.selected;
  const model = document.querySelector("#model-identifier");
''',
)
replace_once(
    "crates/rah-desktop/frontend/status.js",
    '''  document.querySelector("#codex-connection").addEventListener("click", () => {
    void toggleCodexConnection(invoke);
  });
  document.querySelector("#choose-repository").addEventListener("click", async () => {
''',
    '''  document.querySelector("#codex-connection").addEventListener("click", () => {
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
''',
)
replace_once(
    "crates/rah-desktop/frontend/status.js",
    '''  await loadStatus(invoke);
  await replaceTranscript(invoke);
''',
    '''  await refreshTrustedProfileSelection(invoke);
  await loadStatus(invoke);
  await replaceTranscript(invoke);
''',
)

plan = r'''# Task 204 — Desktop Provider-Only Trusted Profile Selection

## Scope

Implement only the configured-intent phase of the v0.17 Desktop Trusted Profile
contract established by Task 202.

Desktop may explicitly select one Trusted Profile while disconnected. Selection
uses the existing hardened `TrustedStaticProfile::load` path and then applies a
Desktop-only provider overlay policy: the profile must declare zero first-party
`capabilities`.

This task does **not** activate a profile.

## Host state

Desktop owns an ephemeral `DesktopTrustedProfileSelection` containing:

- the explicit host-selected source path, retained only in Rust state for a
  future fresh activation load;
- the validated bounded profile ID;
- MCP provider count;
- Process Plugin provider count; and
- expected external Tool count.

The source path never crosses the Desktop IPC boundary. Selection is not
persisted and startup always begins with no selected profile.

## Static validation

Selection delegates source and schema validation to the established
`TrustedStaticProfile::load` implementation, preserving:

- absolute explicit source requirement;
- link/reparse rejection and path-topology checks;
- regular-file, size and UTF-8 bounds;
- duplicate JSON key rejection;
- closed schema and version validation;
- symbolic resource validation;
- exact provider declaration validation; and
- explicit external permission mapping validation.

Desktop then rejects any profile for which
`effective_profile().capabilities` is non-empty, including disabled first-party
capability declarations.

All Trusted Profile load failures are collapsed to a bounded Desktop
`profile_invalid` category. The provider-only product rejection has the closed
`profile_first_party_capabilities_unsupported` category. Raw profile paths and
profile contents are not included in frontend errors.

## Lifecycle

Profile selection and clearing are accepted only when:

- chat is idle; and
- connection state is `NotConnected` or `Error`.

The backend checks this before opening the native picker, again after the picker
returns, and again after static loading immediately before publishing configured
intent. This prevents a profile selection from being installed if Connect wins
a concurrent race.

Selection does not:

- spawn MCP or Process Plugin children;
- construct `EffectiveProfileComposition`;
- construct or connect Codex;
- merge or replace the Desktop first-party `ToolRegistry`;
- advertise external Tools;
- change repository/model authority or generations;
- persist or auto-restore profile activation; or
- add provider retry, restart, hot reload, discovery or network MCP.

A deterministic Windows test deliberately configures an MCP provider whose
absolute executable path does not exist and requires static selection to
succeed. The test demonstrates that provider process existence/startup is not a
Task 204 selection requirement.

## Sanitized frontend state

The new Trusted Profile panel exposes only:

- selected/not selected;
- bounded profile ID;
- MCP provider count;
- Process Plugin provider count; and
- expected external Tool count.

It does not expose the selected source path, executable resources, argv,
environment, provider stderr, endpoints, credentials, private protocol IDs, or
Generic Codex bridge aliases.

The existing Effective Authority snapshot remains observational and unchanged
in Task 204. External provider effectiveness/advertisement remains zero because
Task 204 never activates the selected profile.

## IPC surface

Three closed Desktop commands are added to the existing Tauri capability:

- `trusted_profile_selection`
- `choose_trusted_profile`
- `clear_trusted_profile`

Tauri autogenerated command permissions are retained in the repository after
the Windows build.

## Validation

Required gates:

- `cargo fmt --check`
- `cargo test -p rah-desktop` on Windows
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` on Windows
- `node --check crates/rah-desktop/frontend/status.js`
- `git diff --check`
- unchanged `Cargo.lock`
- ordinary exact-head workspace CI after publication

No new dependency, workspace package, ADR, profile schema, provider protocol,
Generic Codex Tool Bridge, repository authority or release change is intended.

## Next task

Task 205 — Desktop Effective Provider Composition and Lifecycle — may consume
the selected source only by freshly loading it again at explicit Connect. Task
204 selection must never be treated as a durable effective authority object.
'''
Path(
    "docs/plans/2026-09-04-desktop-provider-only-trusted-profile-selection.md"
).write_text(plan, encoding="utf-8")

# Temporary execution scaffolding must not survive in the validated branch tip.
Path(".github/workflows/task204-impl.yml").unlink()
Path(".github/task204_apply.py").unlink()
