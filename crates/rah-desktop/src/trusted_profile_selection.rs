#![cfg(target_os = "windows")]

use std::path::{Path, PathBuf};

use rah_tools::TrustedStaticProfile;
use serde::Serialize;

use crate::effective_authority::{ExternalToolDescriptor, external_tool_descriptors};

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
    external_tools: Vec<ExternalToolDescriptor>,
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
    pub remembered: bool,
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
    pub(crate) fn none(remembered: bool) -> Self {
        Self {
            remembered,
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
    pub(crate) fn load_for_activation(
        &self,
    ) -> Result<TrustedStaticProfile, ProfileSelectionError> {
        load_provider_only_static_profile(&self.source)
    }

    #[must_use]
    pub(crate) fn external_tools(&self) -> &[ExternalToolDescriptor] {
        &self.external_tools
    }

    #[must_use]
    pub(crate) fn presentation(&self) -> TrustedProfilePresentation {
        self.presentation_with_remembered(true)
    }

    #[must_use]
    pub(crate) fn presentation_with_remembered(
        &self,
        remembered: bool,
    ) -> TrustedProfilePresentation {
        // Read the host-only source solely to preserve the invariant locally;
        // no path-derived value crosses the IPC boundary.
        debug_assert!(self.source.is_absolute());
        TrustedProfilePresentation {
            remembered,
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
fn load_provider_only_static_profile(
    source: &Path,
) -> Result<TrustedStaticProfile, ProfileSelectionError> {
    let profile =
        TrustedStaticProfile::load(source).map_err(|_| ProfileSelectionError::InvalidProfile)?;
    if !profile.effective_profile().capabilities.is_empty() {
        return Err(ProfileSelectionError::FirstPartyCapabilities);
    }
    Ok(profile)
}

pub(crate) fn load_provider_only_profile(
    source: PathBuf,
) -> Result<DesktopTrustedProfileSelection, ProfileSelectionError> {
    let profile = load_provider_only_static_profile(&source)?;

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
    let external_tools =
        external_tool_descriptors(&profile).map_err(|_| ProfileSelectionError::InvalidProfile)?;

    Ok(DesktopTrustedProfileSelection {
        source,
        profile_id: profile.effective_profile().profile_id.clone(),
        mcp_provider_count,
        process_plugin_count,
        expected_tool_count,
        external_tools,
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

    use super::{ProfileSelectionError, TrustedProfilePresentation, load_provider_only_profile};
    use crate::effective_authority::SourceKind;

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
                remembered: true,
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
    fn presentation_distinguishes_remembered_only_from_selected_intent() {
        let remembered_only = TrustedProfilePresentation::none(true);
        assert!(remembered_only.remembered);
        assert!(!remembered_only.selected);
        assert_eq!(remembered_only.profile_id, None);
        assert_eq!(remembered_only.expected_tool_count, 0);
        let encoded = serde_json::to_string(&remembered_only).expect("presentation encodes");
        assert!(encoded.contains(r#""remembered":true"#));
        assert!(!encoded.contains("profile"));
    }

    #[test]
    fn selected_presentation_can_report_that_preference_was_forgotten() {
        let fixture = ProfileFile::write(base_profile());
        let selection = load_provider_only_profile(fixture.path().to_path_buf()).unwrap();
        let presentation = selection.presentation_with_remembered(false);
        assert!(!presentation.remembered);
        assert!(presentation.selected);
        let encoded = serde_json::to_string(&presentation).expect("presentation encodes");
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
        let selection = load_provider_only_profile(fixture.path().to_path_buf())
            .expect("static selection must not require provider startup");
        let presentation = selection.presentation();
        assert_eq!(presentation.mcp_provider_count, 1);
        assert_eq!(presentation.expected_tool_count, 1);
        assert_eq!(selection.external_tools().len(), 1);
        assert_eq!(
            selection.external_tools()[0].public_tool_name,
            "mcp.fixture.echo"
        );
        assert_eq!(selection.external_tools()[0].source_kind, SourceKind::Mcp);
        assert_eq!(
            selection.external_tools()[0].permission,
            rah_protocol::PermissionLevel::None
        );
    }

    #[test]
    fn invalid_profile_is_collapsed_to_bounded_selection_error() {
        let fixture = ProfileFile::write(json!({"profile_version": 1}));
        let error = load_provider_only_profile(fixture.path().to_path_buf())
            .expect_err("invalid profile must fail closed");
        assert_eq!(error, ProfileSelectionError::InvalidProfile);
    }
}
