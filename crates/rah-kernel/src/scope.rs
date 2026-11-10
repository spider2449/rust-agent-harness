use std::{
    any::TypeId,
    future::Future,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::{
    BailHook, BoxFuture, Event, KernelHandle, MountOptions, PluginError, PluginId, PluginManifest,
    ScopeError, SerialHook, ServiceKey, ServiceName, WaterfallHook,
    event::{HandlerEntry, HandlerFn, Owner},
    hook::{BailFn, HookEntry, SerialFn, WaterfallFn},
    kernel::{Inner, guarded, in_lifecycle_scope},
    service::{ServiceEntry, resolve},
};

/// A unit of functionality mounted into the kernel.
#[async_trait]
pub trait Plugin: Send + Sync + 'static {
    fn manifest(&self) -> &PluginManifest;

    /// Checks a configuration before the kernel mounts the plugin with it or
    /// replaces its configuration. A refused configuration fails the mount
    /// or update with [`KernelError::InvalidConfig`](crate::KernelError) and
    /// leaves a running instance untouched. It must not have side effects.
    /// The default accepts every configuration; `apply` can still fail.
    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        let _ = config;
        Ok(())
    }

    /// Registers services, hooks, event handlers, tasks and dispose effects.
    /// Registrations become visible only if `apply` returns `Ok`; on error or
    /// panic they are discarded and the scope is disposed.
    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError>;
}

/// A dependency recorded when a plugin injects a service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    pub provider: PluginId,
}

#[derive(Default)]
pub(crate) struct Staged {
    pub services: Vec<(ServiceName, ServiceEntry)>,
    pub hooks: Vec<(TypeId, HookEntry)>,
    pub handlers: Vec<(TypeId, HandlerEntry)>,
    pub bindings: Vec<Binding>,
}

type Effect = Box<dyn FnOnce() -> BoxFuture<()> + Send>;
type Tasks = Arc<Mutex<Option<JoinSet<()>>>>;

/// Resources owned by one activation of a plugin.
pub(crate) struct ScopeRuntime {
    token: CancellationToken,
    tasks: Tasks,
    effects: Vec<Effect>,
}

impl ScopeRuntime {
    fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            tasks: Arc::new(Mutex::new(Some(JoinSet::new()))),
            effects: Vec::new(),
        }
    }

    /// Cancels and drains owned tasks, then runs dispose effects in reverse
    /// registration order. Effect panics are contained. Neither the tasks
    /// nor any one effect is waited for longer than `limit` (ADR 0076);
    /// returns what was left behind.
    pub(crate) async fn dispose(self, limit: Duration) -> Unfinished {
        let tasks = {
            let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
            self.token.cancel();
            tasks.take()
        };
        let mut left = Unfinished::default();
        if let Some(mut tasks) = tasks.filter(|tasks| !tasks.is_empty()) {
            let drained = within(limit, async { while tasks.join_next().await.is_some() {} });
            if drained.await.is_none() {
                // A task that is still here is in synchronous work. Dropping
                // the set aborts it: it ends at its next await point, as the
                // cancelled scope already asked.
                left.tasks = tasks.len();
            }
        }
        for effect in self.effects.into_iter().rev() {
            // A task of its own, so an effect stuck in synchronous work
            // cannot hold up the lifecycle operation that disposes it.
            let mut running = tokio::spawn(in_lifecycle_scope(async move {
                let _ = guarded(effect).await;
            }));
            if within(limit, &mut running).await.is_none() {
                running.abort();
                left.effects += 1;
            }
        }
        left
    }
}

/// Waits for `future`, at most `limit`; `None` when the time is up. The
/// time is kept by a thread of its own, not by the runtime's timer: what
/// this bounds may be synchronous work on the very threads that drive that
/// timer, which then never fires. The thread ends as soon as the wait does.
/// Without a thread to keep the time, the wait has no limit.
async fn within<T>(limit: Duration, future: impl Future<Output = T>) -> Option<T> {
    let (done, waiting) = std::sync::mpsc::channel::<()>();
    let (expire, expired) = tokio::sync::oneshot::channel::<()>();
    let timer = std::thread::Builder::new()
        .name("rah-dispose-limit".into())
        .spawn(move || {
            if waiting.recv_timeout(limit) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
                let _ = expire.send(());
            }
        });
    if timer.is_err() {
        return Some(future.await);
    }
    // Dropped when this returns, which ends the thread.
    let _done = done;
    tokio::select! {
        biased;
        value = future => Some(value),
        Ok(()) = expired => None,
    }
}

/// What a disposal did not wait for.
#[derive(Default)]
pub(crate) struct Unfinished {
    pub tasks: usize,
    pub effects: usize,
}

/// Spawns tasks owned by a plugin scope. Owned tasks are dropped at their next
/// await point once the scope is disposed.
#[derive(Clone)]
pub struct Spawner {
    token: CancellationToken,
    tasks: Tasks,
}

impl Spawner {
    /// Returns `false` without spawning if the scope is already disposed.
    pub fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) -> bool {
        let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(set) = tasks.as_mut() else {
            return false;
        };
        if self.token.is_cancelled() {
            return false;
        }
        while set.try_join_next().is_some() {}
        let token = self.token.clone();
        set.spawn(async move {
            tokio::select! {
                biased;
                () = token.cancelled() => {}
                () = task => {}
            }
        });
        true
    }

    /// A token cancelled when the owning scope is disposed.
    pub fn cancellation(&self) -> CancellationToken {
        self.token.child_token()
    }
}

/// Registration context handed to [`Plugin::apply`].
pub struct Scope {
    pub(crate) inner: Arc<Inner>,
    pub(crate) manifest: PluginManifest,
    pub(crate) options: MountOptions,
    pub(crate) staged: Staged,
    pub(crate) runtime: ScopeRuntime,
}

impl Scope {
    pub(crate) fn new(inner: Arc<Inner>, manifest: PluginManifest, options: MountOptions) -> Self {
        Self {
            inner,
            manifest,
            options,
            staged: Staged::default(),
            runtime: ScopeRuntime::new(),
        }
    }

    pub fn id(&self) -> &PluginId {
        &self.manifest.id
    }

    pub fn config(&self) -> &serde_json::Value {
        &self.options.config
    }

    /// Deserializes the plugin configuration.
    pub fn config_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        T::deserialize(&self.options.config)
    }

    /// A weak handle for invoking hooks, emitting events and looking up
    /// services after `apply` returns.
    pub fn kernel(&self) -> KernelHandle {
        KernelHandle::from_inner(&self.inner)
    }

    /// Provides a service under an instance named after this plugin.
    pub fn provide<S>(&mut self, key: ServiceKey<S>, service: Arc<S>) -> Result<(), ScopeError>
    where
        S: ?Sized + Send + Sync + 'static,
    {
        let instance = self.manifest.id.to_string();
        self.provide_instance(key, instance, service)
    }

    /// Provides a named instance of a service. The service must be declared
    /// in the manifest's `provides`.
    pub fn provide_instance<S>(
        &mut self,
        key: ServiceKey<S>,
        instance: impl Into<String>,
        service: Arc<S>,
    ) -> Result<(), ScopeError>
    where
        S: ?Sized + Send + Sync + 'static,
    {
        let name = key.name();
        if !self.manifest.provides.contains(&name) {
            return Err(ScopeError::UndeclaredProvide(name));
        }
        let entry = ServiceEntry::new(
            instance.into(),
            self.manifest.id.clone(),
            self.inner.next_seq(),
            service,
        );
        {
            let state = self.inner.state();
            let committed = state.services.get(&name).into_iter().flatten();
            let staged = self
                .staged
                .services
                .iter()
                .filter(|(n, _)| n == &name)
                .map(|(_, e)| e);
            for existing in committed.chain(staged) {
                if existing.instance == entry.instance {
                    return Err(ScopeError::DuplicateInstance {
                        service: name,
                        instance: entry.instance,
                    });
                }
                if !existing.same_type(&entry) {
                    return Err(ScopeError::TypeMismatch {
                        service: name,
                        registered: existing.type_name(),
                        requested: entry.type_name(),
                    });
                }
            }
        }
        self.staged.services.push((name, entry));
        Ok(())
    }

    /// Injects a service that must be available.
    pub fn inject<S>(&mut self, key: ServiceKey<S>) -> Result<Arc<S>, ScopeError>
    where
        S: ?Sized + Send + Sync + 'static,
    {
        self.inject_optional(key)?
            .ok_or_else(|| ScopeError::Unavailable(key.name()))
    }

    /// Injects a service if available. The service must be declared in the
    /// manifest's `requires` or `optional`. Injection binds this plugin to the
    /// provider: if the provider stops, this plugin is stopped first. A
    /// service in `requires` also binds without an injection: the plugin is
    /// stopped when the service no longer resolves (ADR 0075).
    pub fn inject_optional<S>(&mut self, key: ServiceKey<S>) -> Result<Option<Arc<S>>, ScopeError>
    where
        S: ?Sized + Send + Sync + 'static,
    {
        let name = key.name();
        if !self.manifest.may_inject(&name) {
            return Err(ScopeError::UndeclaredInject(name));
        }
        let state = self.inner.state();
        let Some(entry) = resolve(state.services.get(&name), self.options.bindings.get(&name))
        else {
            return Ok(None);
        };
        let service = entry.get::<S>(&name)?;
        let binding = Binding {
            provider: entry.provider.clone(),
        };
        drop(state);
        if !self.staged.bindings.contains(&binding) {
            self.staged.bindings.push(binding);
        }
        Ok(Some(service))
    }

    /// Handles events of type `E` until this scope is disposed.
    pub fn on<E: Event>(&mut self, handler: impl Fn(&E) + Send + Sync + 'static) {
        let handler: HandlerFn<E> = Arc::new(handler);
        self.staged.handlers.push((
            TypeId::of::<E>(),
            HandlerEntry {
                owner: Owner::Plugin(self.manifest.id.clone()),
                handler: Arc::new(handler),
            },
        ));
    }

    pub fn hook_bail<H, F, Fut>(&mut self, priority: i32, handler: F)
    where
        H: BailHook,
        F: Fn(H::Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<H::Output>> + Send + 'static,
    {
        let handler: BailFn<H> = Arc::new(move |input| Box::pin(handler(input)));
        self.push_hook::<H>(priority, Arc::new(handler));
    }

    pub fn hook_waterfall<H, F, Fut>(&mut self, priority: i32, handler: F)
    where
        H: WaterfallHook,
        F: Fn(H::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = H::Value> + Send + 'static,
    {
        let handler: WaterfallFn<H> = Arc::new(move |value| Box::pin(handler(value)));
        self.push_hook::<H>(priority, Arc::new(handler));
    }

    pub fn hook_serial<H, F, Fut>(&mut self, priority: i32, handler: F)
    where
        H: SerialHook,
        F: Fn(H::Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let handler: SerialFn<H> = Arc::new(move |input| Box::pin(handler(input)));
        self.push_hook::<H>(priority, Arc::new(handler));
    }

    fn push_hook<H: 'static>(
        &mut self,
        priority: i32,
        handler: Arc<dyn std::any::Any + Send + Sync>,
    ) {
        self.staged.hooks.push((
            TypeId::of::<H>(),
            HookEntry {
                owner: self.manifest.id.clone(),
                priority,
                seq: self.inner.next_seq(),
                handler,
            },
        ));
    }

    pub fn spawner(&self) -> Spawner {
        Spawner {
            token: self.runtime.token.clone(),
            tasks: self.runtime.tasks.clone(),
        }
    }

    /// Spawns a task owned by this scope.
    pub fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) -> bool {
        self.spawner().spawn(task)
    }

    /// Registers cleanup run when this scope is disposed, after owned tasks
    /// are cancelled. Effects run in reverse registration order. An effect
    /// that does not return within the kernel's limit is aborted (ADR 0076).
    pub fn on_dispose<F, Fut>(&mut self, effect: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.runtime
            .effects
            .push(Box::new(move || Box::pin(effect())));
    }
}
