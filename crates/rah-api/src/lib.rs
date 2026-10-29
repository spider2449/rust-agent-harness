//! RAH service contracts (ADR 0036).
//!
//! Plugins depend on this crate and `rah-protocol` only. It re-exports the
//! kernel, defines the service keys and traits plugins provide and consume,
//! the standard hooks, and the [`tools::ToolInvoker`] through which every
//! model-originated tool call must pass.
//!
//! Two ways to use a service:
//! - `Scope::inject` binds statically: the plugin restarts when its provider
//!   goes away.
//! - `KernelHandle::get` resolves on every use: providers can be swapped while
//!   the consumer keeps running. The standard loop resolves the model per turn.

pub mod admin;
pub mod agent_loop;
pub mod config;
pub mod hooks;
pub mod host;
pub mod model;
pub mod retry;
pub mod sandbox;
pub mod scheduler;
pub mod session;
pub mod storage;
pub mod tools;
pub mod undo;

pub use async_trait::async_trait;
pub use rah_kernel as kernel;
pub use rah_kernel::{
    Kernel, KernelHandle, MountOptions, Plugin, PluginError, PluginId, PluginManifest,
    PluginStatus, Scope, ServiceKey,
};
pub use rah_protocol as protocol;
pub use tokio_util::sync::CancellationToken;
