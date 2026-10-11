#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rah_kernel::{Plugin, PluginError, PluginManifest, Scope, ServiceKey};

pub trait Greeter: Send + Sync {
    fn greet(&self) -> String;
}

pub const GREETER: ServiceKey<dyn Greeter> = ServiceKey::new("greeter");

pub struct Fixed(pub String);

impl Greeter for Fixed {
    fn greet(&self) -> String {
        self.0.clone()
    }
}

/// Ordered, shared event log for asserting lifecycle order.
#[derive(Clone, Default)]
pub struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    pub fn push(&self, entry: impl Into<String>) {
        self.0.lock().unwrap().push(entry.into());
    }

    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

type ApplyFn = dyn Fn(&mut Scope) -> Result<(), PluginError> + Send + Sync;

/// A plugin whose `apply` is a synchronous closure.
pub struct FnPlugin {
    manifest: PluginManifest,
    apply: Box<ApplyFn>,
}

#[async_trait]
impl Plugin for FnPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        (self.apply)(scope)
    }
}

pub fn plugin(
    manifest: PluginManifest,
    apply: impl Fn(&mut Scope) -> Result<(), PluginError> + Send + Sync + 'static,
) -> Arc<dyn Plugin> {
    Arc::new(FnPlugin {
        manifest,
        apply: Box::new(apply),
    })
}

/// Provides `greeter` (instance = plugin id) greeting with `config.greeting`
/// or the plugin id; logs `start:<id>` and `dispose:<id>`.
pub fn provider(id: &str, log: &Log) -> Arc<dyn Plugin> {
    let log = log.clone();
    let name = id.to_owned();
    plugin(
        PluginManifest::new(id, "1.0.0").provides(["greeter"]),
        move |scope| {
            let greeting = scope.config()["greeting"]
                .as_str()
                .map_or_else(|| name.clone(), str::to_owned);
            scope.provide(GREETER, Arc::new(Fixed(greeting)))?;
            log.push(format!("start:{name}"));
            let (log, name) = (log.clone(), name.clone());
            scope.on_dispose(move || async move { log.push(format!("dispose:{name}")) });
            Ok(())
        },
    )
}

/// Requires `greeter`; records the injected greeting in `seen`.
pub fn consumer(id: &str, log: &Log, seen: &Log) -> Arc<dyn Plugin> {
    let (log, seen) = (log.clone(), seen.clone());
    let name = id.to_owned();
    plugin(
        PluginManifest::new(id, "1.0.0").requires(["greeter"]),
        move |scope| {
            let greeter = scope.inject(GREETER)?;
            seen.push(format!("{name}={}", greeter.greet()));
            log.push(format!("start:{name}"));
            let (log, name) = (log.clone(), name.clone());
            scope.on_dispose(move || async move { log.push(format!("dispose:{name}")) });
            Ok(())
        },
    )
}

/// Logs a message when dropped; used to observe task cancellation.
pub struct DropGuard(pub Log, pub &'static str);

impl Drop for DropGuard {
    fn drop(&mut self) {
        self.0.push(self.1);
    }
}
