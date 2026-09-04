from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one match in {path}, found {count}: {old[:120]!r}")
    p.write_text(text.replace(old, new, 1), encoding="utf-8")


main = "crates/rah-desktop/src/main.rs"

replace_once(
    main,
    "    PermissionLevel, RequestId, SessionId, ToolContent, ToolInput,\n",
    "    RequestId, SessionId, ToolContent, ToolInput,\n",
)

helper_marker = '''#[cfg(target_os = "windows")]
fn safe_repository_display_name(root: &Path) -> Option<String> {
'''
helper = r'''#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderPublicationRejectionReason {
    Superseded,
    DuplicateOwner,
}

#[cfg(target_os = "windows")]
struct PendingConnectedPublication {
    runtime: Arc<CodexRuntime>,
    activation: Option<DesktopProviderActivation>,
    source: CodexExecutableSource,
    repository_generation: u64,
    model_generation: u64,
    connection_generation: u64,
    repository_fingerprint: Option<String>,
    composition: Arc<DesktopToolComposition>,
    profile_generation: u64,
}

#[cfg(target_os = "windows")]
struct RejectedProviderPublication {
    runtime: Arc<CodexRuntime>,
    activation: Option<DesktopProviderActivation>,
    reason: ProviderPublicationRejectionReason,
}

#[cfg(target_os = "windows")]
fn publish_connected_provider_state(
    state: &DesktopAppState,
    pending: PendingConnectedPublication,
) -> Result<(), RejectedProviderPublication> {
    let mut connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !matches!(*connection, ConnectionState::Connecting) {
        let PendingConnectedPublication {
            runtime,
            activation,
            ..
        } = pending;
        return Err(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::Superseded,
        });
    }

    let mut published_provider = state
        .provider_activation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if published_provider.is_some() {
        let PendingConnectedPublication {
            runtime,
            activation,
            ..
        } = pending;
        return Err(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::DuplicateOwner,
        });
    }

    let PendingConnectedPublication {
        runtime,
        activation,
        source,
        repository_generation,
        model_generation,
        connection_generation,
        repository_fingerprint,
        composition,
        profile_generation,
    } = pending;
    *published_provider = activation;
    *connection = ConnectionState::Connected {
        runtime,
        source,
        repository_generation,
        model_generation,
        connection_generation,
        repository_fingerprint,
        composition,
        profile_generation,
    };
    Ok(())
}

'''
replace_once(main, helper_marker, helper + helper_marker)

old = r'''            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !matches!(*connection, ConnectionState::Connecting) {
                drop(connection);
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "superseded Codex runtime shutdown failed");
                }
                if let Some(activation) = provider_activation.take() {
                    activation.shutdown().await;
                }
                return Err(FrontendError::CodexReconnectRequired);
            }
            let mut published_provider = state
                .provider_activation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if published_provider.is_some() {
                drop(published_provider);
                drop(connection);
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "duplicate provider owner runtime shutdown failed");
                }
                if let Some(activation) = provider_activation.take() {
                    activation.shutdown().await;
                }
                let frontend_error = FrontendError::ProfileActivationFailed;
                *state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    ConnectionState::Error(frontend_error);
                return Err(frontend_error);
            }
            *published_provider = provider_activation.take();
            *connection = ConnectionState::Connected {
                runtime,
                source,
                repository_generation,
                model_generation,
                connection_generation,
                repository_fingerprint,
                composition: Arc::clone(&composition),
                profile_generation,
            };
            drop(published_provider);
            drop(connection);
'''
new = r'''            let pending = PendingConnectedPublication {
                runtime,
                activation: provider_activation.take(),
                source,
                repository_generation,
                model_generation,
                connection_generation,
                repository_fingerprint,
                composition: Arc::clone(&composition),
                profile_generation,
            };
            if let Err(rejected) = publish_connected_provider_state(state.inner(), pending) {
                if let Err(error) = rejected.runtime.shutdown().await {
                    tracing::warn!(error = %error, "rejected Codex runtime shutdown failed");
                }
                if let Some(activation) = rejected.activation {
                    activation.shutdown().await;
                }
                return match rejected.reason {
                    ProviderPublicationRejectionReason::Superseded => {
                        Err(FrontendError::CodexReconnectRequired)
                    }
                    ProviderPublicationRejectionReason::DuplicateOwner => {
                        state.shutdown_provider_activation().await;
                        let frontend_error = FrontendError::ProfileActivationFailed;
                        let mut connection = state
                            .connection
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if matches!(*connection, ConnectionState::Connecting) {
                            *connection = ConnectionState::Error(frontend_error);
                        }
                        Err(frontend_error)
                    }
                };
            }
'''
replace_once(main, old, new)
