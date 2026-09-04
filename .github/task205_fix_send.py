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

# Task 205 clippy cleanup. These rewrites are behavior-neutral: profile
# currentness is required only before publication; the connected state cannot
# change its selected profile while live.
replace_once(
    main,
    '''        composition: Arc<DesktopToolComposition>,
        profile_generation: u64,
    },
''',
    '''        composition: Arc<DesktopToolComposition>,
    },
''',
)
replace_once(
    main,
    '''    composition: Arc<DesktopToolComposition>,
    profile_generation: u64,
}
''',
    '''    composition: Arc<DesktopToolComposition>,
}
''',
)
replace_once(
    main,
    '''        repository_fingerprint,
        composition,
        profile_generation,
    } = pending;
''',
    '''        repository_fingerprint,
        composition,
    } = pending;
''',
)
replace_once(
    main,
    '''        repository_fingerprint,
        composition,
        profile_generation,
    };
''',
    '''        repository_fingerprint,
        composition,
    };
''',
)
replace_once(
    main,
    '''                repository_fingerprint,
                composition: Arc::clone(&composition),
                profile_generation,
            };
''',
    '''                repository_fingerprint,
                composition: Arc::clone(&composition),
            };
''',
)

replace_once(
    main,
    '''#[cfg(target_os = "windows")]
fn connection_activation_publication_is_current(
    captured_repository_generation: u64,
    current_repository_generation: u64,
    captured_model_generation: u64,
    current_model_generation: u64,
    captured_profile_generation: u64,
    current_profile_generation: u64,
    captured_connection_generation: u64,
    current_connection_generation: u64,
) -> bool {
    captured_repository_generation == current_repository_generation
        && captured_model_generation == current_model_generation
        && captured_profile_generation == current_profile_generation
        && captured_connection_generation == current_connection_generation
}
''',
    '''#[cfg(target_os = "windows")]
fn connection_activation_publication_is_current(
    captured: [u64; 4],
    current: [u64; 4],
) -> bool {
    captured == current
}
''',
)
replace_once(
    main,
    '''            let stale = !connection_activation_publication_is_current(
                repository_generation,
                current_repository_generation,
                model_generation,
                current_model_generation,
                profile_generation,
                current_profile_generation,
                connection_generation,
                current_connection_generation,
            );
''',
    '''            let stale = !connection_activation_publication_is_current(
                [
                    repository_generation,
                    model_generation,
                    profile_generation,
                    connection_generation,
                ],
                [
                    current_repository_generation,
                    current_model_generation,
                    current_profile_generation,
                    current_connection_generation,
                ],
            );
''',
)
replace_once(
    main,
    '''    #[test]
    fn activation_publication_requires_repository_model_profile_and_connection_currentness() {
        assert!(super::connection_activation_publication_is_current(
            4, 4, 5, 5, 6, 6, 9, 9
        ));
        assert!(!super::connection_activation_publication_is_current(
            4, 5, 5, 5, 6, 6, 9, 9
        ));
        assert!(!super::connection_activation_publication_is_current(
            4, 4, 5, 7, 6, 6, 9, 9
        ));
        assert!(!super::connection_activation_publication_is_current(
            4, 4, 5, 5, 6, 8, 9, 9
        ));
        assert!(!super::connection_activation_publication_is_current(
            4, 4, 5, 5, 6, 6, 9, 10
        ));
    }
''',
    '''    #[test]
    fn activation_publication_requires_repository_model_profile_and_connection_currentness() {
        assert!(super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 5, 6, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [5, 5, 6, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 7, 6, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 5, 8, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 5, 6, 10],
        ));
    }
''',
)

replace_once(
    main,
    '''#[cfg(target_os = "windows")]
fn desktop_tool_composition(
    repository: Option<&DesktopRepository>,
    commit_tool: Option<Arc<RepositoryCommitTool>>,
) -> Result<Arc<DesktopToolComposition>, ToolError> {
    let registry = desktop_tool_registry(repository, commit_tool.clone())?;
    Ok(desktop_tool_composition_from_registry(
        registry,
        repository,
        commit_tool.is_some(),
    ))
}

''',
    '',
)

replace_once(
    main,
    ''') -> Result<(), RejectedProviderPublication> {
''',
    ''') -> Result<(), Box<RejectedProviderPublication>> {
''',
)
replace_once(
    main,
    '''        return Err(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::Superseded,
        });
''',
    '''        return Err(Box::new(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::Superseded,
        }));
''',
)
replace_once(
    main,
    '''        return Err(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::DuplicateOwner,
        });
''',
    '''        return Err(Box::new(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::DuplicateOwner,
        }));
''',
)
replace_once(
    main,
    '''            if let Err(rejected) = publish_connected_provider_state(state.inner(), pending) {
                if let Err(error) = rejected.runtime.shutdown().await {
''',
    '''            if let Err(rejected) = publish_connected_provider_state(state.inner(), pending) {
                let RejectedProviderPublication {
                    runtime,
                    activation,
                    reason,
                } = *rejected;
                if let Err(error) = runtime.shutdown().await {
''',
)
replace_once(
    main,
    '''                if let Some(activation) = rejected.activation {
                    activation.shutdown().await;
                }
                return match rejected.reason {
''',
    '''                if let Some(activation) = activation {
                    activation.shutdown().await;
                }
                return match reason {
''',
)
