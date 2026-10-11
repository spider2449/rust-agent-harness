//! RAH plugin kernel.
//!
//! The kernel owns plugin lifecycle, typed service registration and injection,
//! dependency resolution, hook chains, an event bus and scoped resources. It
//! contains no agent domain logic: models, loops, tools, sessions and policy
//! are all plugins built on top of it (ADR 0036).
//!
//! Lifecycle: a mounted plugin is `Pending` until every service in its
//! `requires` list resolves, then `Starting` while its [`Plugin::apply`] runs,
//! then `Active`. When a service an active plugin is bound to disappears, the
//! plugin is stopped (dependents before providers) and becomes `Pending`
//! again; it restarts automatically once its requirements resolve.

mod error;
mod event;
mod hook;
mod kernel;
mod manifest;
mod scope;
mod service;

pub use error::{HookError, KernelError, PluginError, ScopeError};
pub use event::{Event, PluginStatusChanged, Subscription};
pub use hook::{BailHook, SerialHook, WaterfallHook};
pub use kernel::{Kernel, KernelHandle};
pub use manifest::{KERNEL_API, MountOptions, PluginId, PluginManifest, PluginStatus, ServiceName};
pub use scope::{Plugin, Scope, Spawner};
pub use service::ServiceKey;

/// Boxed `Send` future used by hook and dispose handlers.
pub type BoxFuture<T> = futures::future::BoxFuture<'static, T>;
