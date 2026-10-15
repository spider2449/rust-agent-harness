use crate::{PluginId, ServiceName};

/// Error returned by a plugin's `apply`. Plugins are heterogeneous, so the
/// kernel keeps the source error opaque and records its message as the
/// plugin's `Failed` status.
pub type PluginError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Lifecycle operation failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KernelError {
    #[error("plugin `{0}` is already mounted")]
    DuplicatePlugin(PluginId),
    #[error("plugin `{0}` is not mounted")]
    UnknownPlugin(PluginId),
    #[error("invalid manifest for plugin `{id}`: {reason}")]
    InvalidManifest { id: PluginId, reason: String },
    #[error("invalid configuration for plugin `{id}`: {reason}")]
    InvalidConfig { id: PluginId, reason: String },
    #[error("plugin `{id}` targets kernel API {found}; this kernel provides {expected}")]
    IncompatibleApi {
        id: PluginId,
        found: u32,
        expected: u32,
    },
    #[error("lifecycle operations cannot run inside a plugin's apply or dispose")]
    Reentrant,
    #[error("the kernel has been shut down")]
    ShutDown,
}

/// Failures of service registration or injection inside a [`crate::Scope`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    #[error("service `{0}` is not declared in the manifest's `provides`")]
    UndeclaredProvide(ServiceName),
    #[error("service `{0}` is not declared in the manifest's `requires` or `optional`")]
    UndeclaredInject(ServiceName),
    #[error("service `{0}` is unavailable")]
    Unavailable(ServiceName),
    #[error("service `{service}` is registered as `{registered}`, requested as `{requested}`")]
    TypeMismatch {
        service: ServiceName,
        registered: &'static str,
        requested: &'static str,
    },
    #[error("service `{service}` instance `{instance}` is already provided")]
    DuplicateInstance {
        service: ServiceName,
        instance: String,
    },
}

/// Hook invocation failures. Callers that make security decisions must treat
/// any error as a denial (fail closed).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HookError {
    #[error("a `{hook}` handler owned by `{owner}` panicked")]
    Panicked { hook: &'static str, owner: String },
    #[error("the kernel is no longer available")]
    KernelGone,
}
