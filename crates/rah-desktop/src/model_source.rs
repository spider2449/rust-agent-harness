//! Ephemeral host-owned source evidence. No permissions or provider compatibility proof.
use crate::{
    DesktopAppState, DesktopModelProvider, DesktopModelSelection, FrontendError,
    ModelSelectionMode,
    runtime_model_state::{RuntimeAdapterIdentity, identity},
};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Context {
    adapter: RuntimeAdapterIdentity,
    selection: Option<DesktopModelSelection>,
    native_model: Option<String>,
    artifact_selector: Option<std::ffi::OsString>,
}
impl Context {
    fn capture(state: &DesktopAppState) -> Self {
        let adapter = identity(state.runtime_adapter);
        Self {
            adapter,
            selection: (adapter == RuntimeAdapterIdentity::Codex).then(|| {
                state
                    .model
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .selection
                    .clone()
            }),
            native_model: (adapter == RuntimeAdapterIdentity::OpenAi)
                .then(|| state.openai_configured_model.clone())
                .flatten(),
            artifact_selector: (adapter == RuntimeAdapterIdentity::Codex)
                .then(|| std::env::var_os("RAH_CODEX_EXECUTABLE"))
                .flatten(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SourceState {
    Loading,
    AdvertisedCatalog { models: Vec<String> },
    EmptyCatalog,
    Unavailable,
    Error,
    RuntimeDefault { models: Vec<String> },
    Configured { model: String },
    NoRuntime,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Eligibility {
    AdvertisedUnverified,
    CustomUnverified,
    InheritedUnverified,
    Configured,
    Loading,
    NoSelection,
    InvalidConfiguration,
    AdvertisedAbsent,
    SourceUnavailable,
    KnownRejection,
    NoRuntime,
}
impl Eligibility {
    pub(crate) fn allowed(&self) -> bool {
        matches!(
            self,
            Self::AdvertisedUnverified
                | Self::CustomUnverified
                | Self::InheritedUnverified
                | Self::Configured
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub generation: u64,
    pub request_id: u64,
    pub adapter: RuntimeAdapterIdentity,
    pub admitted_artifact_sha256: Option<String>,
    pub codex_upstream_provider: Option<DesktopModelProvider>,
    pub selection_mode: Option<ModelSelectionMode>,
    pub selected_model: Option<String>,
    pub source: SourceState,
    pub eligibility: Eligibility,
    pub compatibility: &'static str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ticket {
    generation: u64,
    request_id: u64,
}
#[derive(Default)]
pub(crate) struct Owner {
    context: Option<Context>,
    generation: u64,
    request_id: u64,
    source: Option<SourceState>,
    artifact: Option<String>,
    // Only independent exact-context evidence may populate this field.
    rejected_model: Option<String>,
}
impl Owner {
    fn bind(&mut self, context: Context) {
        if self.context.as_ref() != Some(&context) {
            // Relabeling the same exact model as Custom is not new compatibility
            // context and cannot erase independent rejection evidence.
            let rejection = self
                .context
                .as_ref()
                .filter(|previous| {
                    let mut normalized = (*previous).clone();
                    if let (Some(old), Some(new)) = (&mut normalized.selection, &context.selection)
                    {
                        old.model_selection_mode = new.model_selection_mode;
                    }
                    normalized == context
                })
                .and(self.rejected_model.clone());
            self.revoke();
            self.context = Some(context);
            self.rejected_model = rejection;
        }
    }
    pub(crate) fn revoke(&mut self) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("source generation exhausted");
        self.source = None;
        self.artifact = None;
        self.rejected_model = None;
    }
    fn begin(&mut self, context: Context) -> Ticket {
        self.bind(context);
        self.request_id = self
            .request_id
            .checked_add(1)
            .expect("source request ID exhausted");
        self.source = Some(SourceState::Loading);
        Ticket {
            generation: self.generation,
            request_id: self.request_id,
        }
    }
    fn current(&self, ticket: Ticket) -> bool {
        ticket
            == (Ticket {
                generation: self.generation,
                request_id: self.request_id,
            })
    }
    fn publish(&mut self, ticket: Ticket, artifact: Option<String>, source: SourceState) -> bool {
        if !self.current(ticket) {
            return false;
        }
        if self.artifact.is_some() && self.artifact != artifact {
            self.generation = self
                .generation
                .checked_add(1)
                .expect("source generation exhausted");
            self.rejected_model = None;
        }
        self.artifact = artifact;
        self.source = Some(source);
        true
    }
    fn view(&self, context: &Context) -> Snapshot {
        let fresh = self.context.as_ref() == Some(context);
        let source = if fresh {
            self.source.clone().unwrap_or(SourceState::Unavailable)
        } else {
            SourceState::Unavailable
        };
        let eligibility = eligibility(
            context,
            &source,
            fresh.then_some(self.rejected_model.as_deref()).flatten(),
        );
        Snapshot {
            generation: self.generation,
            request_id: self.request_id,
            adapter: context.adapter,
            admitted_artifact_sha256: fresh.then(|| self.artifact.clone()).flatten(),
            codex_upstream_provider: context.selection.as_ref().map(|s| s.provider),
            selection_mode: context
                .selection
                .as_ref()
                .and_then(|s| s.model_selection_mode),
            selected_model: if context.adapter == RuntimeAdapterIdentity::OpenAi {
                context.native_model.clone()
            } else {
                context.selection.as_ref().and_then(|s| s.model.clone())
            },
            source,
            eligibility,
            compatibility: if fresh && self.rejected_model.is_some() {
                "rejected"
            } else {
                "unverified"
            },
        }
    }
}
pub(crate) fn configuration_changed(state: &DesktopAppState) {
    let context = Context::capture(state);
    state
        .model_source
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .bind(context);
}
fn eligibility(context: &Context, source: &SourceState, rejected: Option<&str>) -> Eligibility {
    match context.adapter {
        RuntimeAdapterIdentity::None => Eligibility::NoRuntime,
        RuntimeAdapterIdentity::OpenAi => match source {
            SourceState::Configured { model }
                if crate::validate_model_identifier(model).is_ok() =>
            {
                Eligibility::Configured
            }
            _ => Eligibility::SourceUnavailable,
        },
        RuntimeAdapterIdentity::Codex => {
            let Some(selection) = &context.selection else {
                return Eligibility::NoSelection;
            };
            if selection.validate().is_err() {
                return Eligibility::InvalidConfiguration;
            }
            if matches!(source, SourceState::Loading) {
                return Eligibility::Loading;
            }
            if selection
                .model
                .as_deref()
                .is_some_and(|id| Some(id) == rejected)
            {
                return Eligibility::KnownRejection;
            }
            if selection.provider == DesktopModelProvider::Inherit {
                return if matches!(source, SourceState::RuntimeDefault { .. }) {
                    Eligibility::InheritedUnverified
                } else {
                    Eligibility::SourceUnavailable
                };
            }
            match selection.model_selection_mode {
                Some(ModelSelectionMode::Custom) => Eligibility::CustomUnverified,
                Some(ModelSelectionMode::Advertised) => match source {
                    SourceState::AdvertisedCatalog { models }
                        if selection
                            .model
                            .as_ref()
                            .is_some_and(|id| models.contains(id)) =>
                    {
                        Eligibility::AdvertisedUnverified
                    }
                    SourceState::AdvertisedCatalog { .. } | SourceState::EmptyCatalog => {
                        Eligibility::AdvertisedAbsent
                    }
                    _ => Eligibility::SourceUnavailable,
                },
                None => Eligibility::NoSelection,
            }
        }
    }
}

/// Bounded IPC read/refresh. The request future owns process teardown; no detached task.
pub(crate) async fn refresh(state: &DesktopAppState, force: bool) -> Snapshot {
    let cached = {
        let _lifecycle = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let context = Context::capture(state);
        state
            .model_source
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .view(&context)
    };
    let artifact_changed =
        cached.admitted_artifact_sha256.is_some() && !artifact_current(&cached).await;
    let (context, ticket) = {
        let _lifecycle = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let context = Context::capture(state);
        let mut owner = state
            .model_source
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        owner.bind(context.clone());
        if artifact_changed
            && owner.generation == cached.generation
            && owner.request_id == cached.request_id
        {
            owner.revoke();
        }
        if !force && owner.source.is_some() {
            return owner.view(&context);
        }
        let ticket = owner.begin(context.clone());
        (context, ticket)
    };
    let (artifact, source) = resolve(&context).await;
    let _lifecycle = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current_context = Context::capture(state);
    let mut owner = state
        .model_source
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    owner.bind(current_context.clone());
    owner.publish(ticket, artifact, source);
    owner.view(&current_context)
}
async fn resolve(context: &Context) -> (Option<String>, SourceState) {
    match context.adapter {
        RuntimeAdapterIdentity::None => (None, SourceState::NoRuntime),
        RuntimeAdapterIdentity::OpenAi => (
            None,
            context
                .native_model
                .as_ref()
                .filter(|m| crate::validate_model_identifier(m).is_ok())
                .map(|model| SourceState::Configured {
                    model: model.clone(),
                })
                .unwrap_or(SourceState::Unavailable),
        ),
        RuntimeAdapterIdentity::Codex => {
            #[cfg(feature = "provider-codex")]
            {
                let result = resolve_codex(context).await;
                result.unwrap_or((None, SourceState::Error))
            }
            #[cfg(not(feature = "provider-codex"))]
            {
                (None, SourceState::Unavailable)
            }
        }
    }
}
#[cfg(feature = "provider-codex")]
async fn resolve_codex(context: &Context) -> Result<(Option<String>, SourceState), FrontendError> {
    use rah_runtime::experimental::ConfiguredRuntimeFactory;
    use rah_runtime_codex::experimental::CodexFactory;
    let (path, artifact) = tokio::task::spawn_blocking(|| {
        let selection =
            crate::resolve_codex_executable().map_err(|_| FrontendError::CodexBaselineInvalid)?;
        CodexFactory::measured_artifact(std::path::Path::new(&selection.executable))
            .map_err(|e| crate::runtime_selection::frontend_failure(&e))
    })
    .await
    .map_err(|_| FrontendError::ModelCatalogUnavailable)??;
    let factory = CodexFactory::new(path, rah_runtime_codex::CodexModelProvider::OpenAi);
    if context
        .selection
        .as_ref()
        .is_some_and(|s| s.provider == DesktopModelProvider::Inherit)
    {
        let instance = factory
            .create()
            .await
            .map_err(|e| crate::runtime_selection::frontend_failure(&e))?;
        instance
            .shutdown()
            .await
            .map_err(|e| crate::runtime_selection::frontend_failure(&e))?;
        let models = factory
            .advertised_catalog()
            .await
            .map(|c| c.models)
            .unwrap_or_default();
        return Ok((Some(artifact), SourceState::RuntimeDefault { models }));
    }
    let catalog = factory
        .advertised_catalog()
        .await
        .map_err(|e| crate::runtime_selection::frontend_failure(&e))?;
    let source = if catalog.models.is_empty() {
        SourceState::EmptyCatalog
    } else {
        SourceState::AdvertisedCatalog {
            models: catalog.models,
        }
    };
    Ok((Some(artifact), source))
}

pub(crate) fn precheck(state: &DesktopAppState, expected: &Snapshot) -> Result<(), FrontendError> {
    let context = Context::capture(state);
    let owner = state
        .model_source
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = owner.view(&context);
    if now.generation != expected.generation
        || now.request_id != expected.request_id
        || now.admitted_artifact_sha256 != expected.admitted_artifact_sha256
        || !now.eligibility.allowed()
    {
        return Err(FrontendError::ModelConfigurationInvalid);
    }
    Ok(())
}

pub(crate) async fn artifact_current(snapshot: &Snapshot) -> bool {
    if snapshot.adapter != RuntimeAdapterIdentity::Codex {
        return true;
    }
    #[cfg(feature = "provider-codex")]
    {
        let expected = snapshot.admitted_artifact_sha256.clone();
        tokio::task::spawn_blocking(move || {
            let Ok(selection) = crate::resolve_codex_executable() else {
                return false;
            };
            rah_runtime_codex::experimental::CodexFactory::measured_artifact(std::path::Path::new(
                &selection.executable,
            ))
            .is_ok_and(|(_, fingerprint)| Some(fingerprint) == expected)
        })
        .await
        .unwrap_or(false)
    }
    #[cfg(not(feature = "provider-codex"))]
    {
        false
    }
}

// Caller holds lifecycle_coordination so configuration and source share one context.
pub(crate) fn current(state: &DesktopAppState) -> Snapshot {
    let context = Context::capture(state);
    let mut owner = state
        .model_source
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    owner.bind(context.clone());
    owner.view(&context)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn codex(id: &str, mode: ModelSelectionMode) -> Context {
        Context {
            adapter: RuntimeAdapterIdentity::Codex,
            selection: Some(DesktopModelSelection {
                provider: DesktopModelProvider::OpenAi,
                model: Some(id.into()),
                model_selection_mode: Some(mode),
                llama_cpp_endpoint: None,
            }),
            native_model: None,
            artifact_selector: None,
        }
    }
    // Catalog-relative observation from 0.157.1; no permanent version/model rule.
    fn catalog() -> SourceState {
        SourceState::AdvertisedCatalog {
            models: vec!["gpt-6-astra".into(), "gpt-6-luna".into()],
        }
    }
    fn ready(owner: &mut Owner, context: &Context, source: SourceState) -> Snapshot {
        let ticket = owner.begin(context.clone());
        assert!(owner.publish(
            ticket,
            (context.adapter == RuntimeAdapterIdentity::Codex).then(|| "admitted-file-hash".into()),
            source
        ));
        owner.view(context)
    }
    #[test]
    fn stale_success_cannot_change_catalog_eligibility_error_or_loading() {
        let a = codex("gpt-6.1-sol", ModelSelectionMode::Advertised);
        let b = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        let old = owner.begin(a);
        let expected = ready(&mut owner, &b, catalog());
        assert!(!owner.publish(old, Some("old".into()), SourceState::EmptyCatalog));
        assert_eq!(owner.view(&b), expected);
        assert_eq!(expected.eligibility, Eligibility::AdvertisedUnverified);
    }
    #[test]
    fn stale_error_cannot_replace_new_success() {
        let a = codex("a", ModelSelectionMode::Advertised);
        let b = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        let old = owner.begin(a);
        let expected = ready(&mut owner, &b, catalog());
        assert!(!owner.publish(old, None, SourceState::Error));
        assert_eq!(owner.view(&b), expected);
    }
    #[test]
    fn newest_same_generation_request_owns_success_error_and_loading_cleanup() {
        let context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        let old = owner.begin(context.clone());
        let newer = owner.begin(context.clone());
        assert_eq!(old.generation, newer.generation);
        assert!(newer.request_id > old.request_id);
        assert_eq!(owner.view(&context).eligibility, Eligibility::Loading);
        assert!(owner.publish(newer, Some("hash".into()), catalog()));
        let expected = owner.view(&context);
        for source in [
            SourceState::Loading,
            SourceState::Error,
            SourceState::EmptyCatalog,
        ] {
            assert!(!owner.publish(old, None, source));
            assert_eq!(owner.view(&context), expected);
        }
    }
    #[test]
    fn loading_withdraws_previous_normal_validity() {
        let context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        assert!(ready(&mut owner, &context, catalog()).eligibility.allowed());
        owner.begin(context.clone());
        assert_eq!(owner.view(&context).source, SourceState::Loading);
        assert!(!owner.view(&context).eligibility.allowed());
    }
    #[test]
    fn restored_advertised_requires_fresh_membership() {
        let context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        owner.bind(context.clone());
        assert!(!owner.view(&context).eligibility.allowed());
        assert_eq!(
            ready(&mut owner, &context, catalog()).eligibility,
            Eligibility::AdvertisedUnverified
        );
    }
    #[test]
    fn gpt_6_1_sol_absent_fixture_blocks_without_becoming_custom() {
        let context = codex("gpt-6.1-sol", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        let view = ready(&mut owner, &context, catalog());
        assert_eq!(view.eligibility, Eligibility::AdvertisedAbsent);
        assert!(!view.eligibility.allowed());
        assert_eq!(
            context.selection.unwrap().model_selection_mode,
            Some(ModelSelectionMode::Advertised)
        );
        // This owner owns no runtime conversation/turn and cannot begin inference.
    }
    #[test]
    fn empty_error_unavailable_are_distinct_and_block_advertised() {
        let context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        for source in [
            SourceState::EmptyCatalog,
            SourceState::Error,
            SourceState::Unavailable,
        ] {
            let mut owner = Owner::default();
            let view = ready(&mut owner, &context, source.clone());
            assert_eq!(view.source, source);
            assert!(!view.eligibility.allowed());
        }
    }
    #[test]
    fn restored_custom_remains_custom_even_when_advertised() {
        let context = codex("gpt-6-astra", ModelSelectionMode::Custom);
        let mut owner = Owner::default();
        let view = ready(&mut owner, &context, catalog());
        assert_eq!(view.eligibility, Eligibility::CustomUnverified);
        assert_eq!(view.compatibility, "unverified");
        assert_eq!(
            context.selection.unwrap().model_selection_mode,
            Some(ModelSelectionMode::Custom)
        );
    }
    #[test]
    fn custom_valid_structure_is_independent_of_catalog_availability() {
        let context = codex("local/model:tag", ModelSelectionMode::Custom);
        for source in [
            catalog(),
            SourceState::EmptyCatalog,
            SourceState::Error,
            SourceState::Unavailable,
        ] {
            assert_eq!(
                ready(&mut Owner::default(), &context, source).eligibility,
                Eligibility::CustomUnverified
            );
        }
        for id in ["", " leading", "trailing ", "line\nbreak", "nul\0value"] {
            let context = codex(id, ModelSelectionMode::Custom);
            assert_eq!(
                ready(&mut Owner::default(), &context, catalog()).eligibility,
                Eligibility::InvalidConfiguration
            );
        }
        let context = codex(&"x".repeat(257), ModelSelectionMode::Custom);
        assert!(
            !ready(&mut Owner::default(), &context, catalog())
                .eligibility
                .allowed()
        );
    }
    #[test]
    fn inherit_requires_no_explicit_id_and_explicit_provider_enables_model() {
        let mut context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        context.selection = Some(DesktopModelSelection::default());
        assert_eq!(
            ready(
                &mut Owner::default(),
                &context,
                SourceState::RuntimeDefault { models: vec![] }
            )
            .eligibility,
            Eligibility::InheritedUnverified
        );
        context.selection.as_mut().unwrap().model = Some("gpt-6-astra".into());
        context.selection.as_mut().unwrap().model_selection_mode =
            Some(ModelSelectionMode::Advertised);
        assert_eq!(
            ready(&mut Owner::default(), &context, catalog()).eligibility,
            Eligibility::InvalidConfiguration
        );
        context.selection.as_mut().unwrap().provider = DesktopModelProvider::OpenAi;
        assert!(
            ready(&mut Owner::default(), &context, catalog())
                .eligibility
                .allowed()
        );
    }
    #[test]
    fn provider_switch_rebinds_identical_catalog_and_clears_rejection() {
        let mut context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        let initial = ready(&mut owner, &context, catalog());
        owner.rejected_model = Some("gpt-6-astra".into());
        assert_eq!(
            owner.view(&context).eligibility,
            Eligibility::KnownRejection
        );
        context.selection.as_mut().unwrap().provider = DesktopModelProvider::Ollama;
        let view = ready(&mut owner, &context, catalog());
        assert!(view.generation > initial.generation);
        assert_eq!(view.compatibility, "unverified");
        assert!(view.eligibility.allowed());
    }
    #[test]
    fn artifact_change_advances_generation_and_clears_rejection() {
        let context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        let first = ready(&mut owner, &context, catalog());
        owner.rejected_model = Some("gpt-6-astra".into());
        let ticket = owner.begin(context.clone());
        assert!(owner.publish(
            ticket,
            Some("different-admitted-artifact".into()),
            catalog()
        ));
        assert!(owner.view(&context).generation > first.generation);
        assert_eq!(owner.view(&context).compatibility, "unverified");
    }
    #[test]
    fn exact_context_rejection_blocks_both_modes_then_model_change_clears() {
        for mode in [ModelSelectionMode::Advertised, ModelSelectionMode::Custom] {
            let mut context = codex("gpt-6-astra", mode);
            let mut owner = Owner::default();
            ready(&mut owner, &context, catalog());
            owner.rejected_model = Some("gpt-6-astra".into());
            assert_eq!(
                owner.view(&context).eligibility,
                Eligibility::KnownRejection
            );
            context.selection.as_mut().unwrap().model = Some("gpt-6-luna".into());
            assert!(ready(&mut owner, &context, catalog()).eligibility.allowed());
        }
    }
    #[test]
    fn custom_relabel_does_not_bypass_exact_context_rejection() {
        let mut context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        ready(&mut owner, &context, catalog());
        owner.rejected_model = Some("gpt-6-astra".into());
        context.selection.as_mut().unwrap().model_selection_mode = Some(ModelSelectionMode::Custom);
        assert_eq!(
            ready(&mut owner, &context, catalog()).eligibility,
            Eligibility::KnownRejection
        );
    }
    #[tokio::test]
    async fn native_configured_only_resolution_and_none_have_no_codex_leakage() {
        let native = Context {
            adapter: RuntimeAdapterIdentity::OpenAi,
            selection: None,
            native_model: Some("configured-only".into()),
            artifact_selector: None,
        };
        let (artifact, source) = resolve(&native).await;
        assert_eq!(artifact, None);
        let view = ready(&mut Owner::default(), &native, source);
        assert_eq!(
            view.source,
            SourceState::Configured {
                model: "configured-only".into()
            }
        );
        assert_eq!(view.codex_upstream_provider, None);
        assert_eq!(view.eligibility, Eligibility::Configured);
        let missing = Context {
            native_model: None,
            ..native
        };
        let (_, source) = resolve(&missing).await;
        assert!(
            !ready(&mut Owner::default(), &missing, source)
                .eligibility
                .allowed()
        );
        let none = Context {
            adapter: RuntimeAdapterIdentity::None,
            ..missing
        };
        let (_, source) = resolve(&none).await;
        assert_eq!(source, SourceState::NoRuntime);
        assert_eq!(
            ready(&mut Owner::default(), &none, source).eligibility,
            Eligibility::NoRuntime
        );
        // These branches contain no factory, catalog, credentials or HTTP client.
    }
    #[test]
    fn adapter_roundtrip_preserves_inactive_preference_and_requires_new_membership() {
        for mode in [ModelSelectionMode::Advertised, ModelSelectionMode::Custom] {
            let context = codex("gpt-6-astra", mode);
            let preference = context.selection.clone();
            let mut owner = Owner::default();
            let old = owner.begin(context.clone());
            let native = Context {
                adapter: RuntimeAdapterIdentity::OpenAi,
                selection: None,
                native_model: Some("native-only".into()),
                artifact_selector: None,
            };
            let expected = ready(
                &mut owner,
                &native,
                SourceState::Configured {
                    model: "native-only".into(),
                },
            );
            assert!(!owner.publish(old, Some("old".into()), catalog()));
            assert_eq!(owner.view(&native), expected);
            owner.begin(context.clone());
            assert_eq!(owner.view(&context).eligibility, Eligibility::Loading);
            assert_eq!(context.selection, preference);
            let restored = ready(&mut owner, &context, catalog());
            assert_eq!(
                restored.eligibility,
                if mode == ModelSelectionMode::Custom {
                    Eligibility::CustomUnverified
                } else {
                    Eligibility::AdvertisedUnverified
                }
            );
        }
    }
    #[test]
    fn disconnect_revokes_late_publication_and_loading() {
        let context = codex("gpt-6-astra", ModelSelectionMode::Advertised);
        let mut owner = Owner::default();
        let old = owner.begin(context.clone());
        owner.revoke();
        assert!(!owner.publish(old, Some("old".into()), catalog()));
        assert_eq!(owner.view(&context).source, SourceState::Unavailable);
        assert!(ready(&mut owner, &context, catalog()).eligibility.allowed());
    }
}
