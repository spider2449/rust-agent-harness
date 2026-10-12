use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet, HashMap},
    panic::AssertUnwindSafe,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::FutureExt;

use crate::{
    BailHook, BoxFuture, Event, HookError, KernelError, MountOptions, Plugin, PluginId,
    PluginManifest, PluginStatus, PluginStatusChanged, Scope, SerialHook, ServiceKey, ServiceName,
    Subscription, WaterfallHook,
    event::{HandlerEntry, HandlerFn, Owner},
    hook::{BailFn, HookEntry, SerialFn, WaterfallFn, insert_sorted},
    scope::{Binding, ScopeRuntime},
    service::{ServiceEntry, resolve},
};

tokio::task_local! {
    /// Set while a plugin's apply or dispose runs, to reject re-entrant
    /// lifecycle operations instead of deadlocking.
    static LIFECYCLE: ();
}

fn in_lifecycle() -> bool {
    LIFECYCLE.try_with(|_| ()).is_ok()
}

/// Runs a handler, converting a panic (at call or during polling) into `Err`.
pub(crate) async fn guarded<T>(call: impl FnOnce() -> BoxFuture<T>) -> Result<T, ()> {
    let future = std::panic::catch_unwind(AssertUnwindSafe(call)).map_err(|_| ())?;
    AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(|_| ())
}

pub(crate) struct Inner {
    /// Serializes lifecycle operations; `true` once shut down.
    ops: tokio::sync::Mutex<bool>,
    state: Mutex<State>,
    seq: AtomicU64,
}

impl Inner {
    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }
}

#[derive(Default)]
pub(crate) struct State {
    plugins: BTreeMap<PluginId, Entry>,
    pub(crate) services: BTreeMap<ServiceName, Vec<ServiceEntry>>,
    hooks: HashMap<TypeId, Vec<HookEntry>>,
    pub(crate) handlers: HashMap<TypeId, Vec<HandlerEntry>>,
}

struct Entry {
    plugin: Arc<dyn Plugin>,
    manifest: PluginManifest,
    options: MountOptions,
    status: PluginStatus,
    mount_seq: u64,
    active: Option<Active>,
}

impl Entry {
    fn waiting(&self) -> bool {
        matches!(
            self.status,
            PluginStatus::Pending { .. } | PluginStatus::DependencyCycle { .. }
        )
    }
}

struct Active {
    activation_seq: u64,
    bindings: Vec<Binding>,
    runtime: ScopeRuntime,
}

type Changes = Vec<PluginStatusChanged>;

fn set_status(id: &PluginId, entry: &mut Entry, status: PluginStatus, changes: &mut Changes) {
    if entry.status != status {
        entry.status = status.clone();
        changes.push(PluginStatusChanged {
            plugin: id.clone(),
            status,
        });
    }
}

fn missing(state: &State, entry: &Entry) -> Vec<ServiceName> {
    entry
        .manifest
        .requires
        .iter()
        .filter(|s| resolve(state.services.get(*s), entry.options.bindings.get(*s)).is_none())
        .cloned()
        .collect()
}

/// The plugin kernel. Cloning is not provided; share a [`KernelHandle`].
pub struct Kernel {
    inner: Arc<Inner>,
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

impl Kernel {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                ops: tokio::sync::Mutex::new(false),
                state: Mutex::new(State::default()),
                seq: AtomicU64::new(0),
            }),
        }
    }

    pub fn handle(&self) -> KernelHandle {
        KernelHandle::from_inner(&self.inner)
    }

    /// Mounts a plugin. It starts immediately if its requirements resolve,
    /// otherwise it waits as `Pending`. Returns the resulting status.
    pub async fn mount(
        &self,
        plugin: Arc<dyn Plugin>,
        options: MountOptions,
    ) -> Result<PluginStatus, KernelError> {
        mount(&self.inner, plugin, options).await
    }

    /// Stops a plugin (dependents first) and removes it.
    pub async fn unmount(&self, id: &PluginId) -> Result<(), KernelError> {
        unmount(&self.inner, id).await
    }

    /// Replaces a plugin's configuration and restarts it and its dependents.
    pub async fn update_config(
        &self,
        id: &PluginId,
        config: serde_json::Value,
    ) -> Result<PluginStatus, KernelError> {
        restart(&self.inner, id, Some(config)).await
    }

    /// Restarts a plugin (for example after `Failed`) and its dependents.
    pub async fn restart(&self, id: &PluginId) -> Result<PluginStatus, KernelError> {
        restart(&self.inner, id, None).await
    }

    pub fn status(&self, id: &PluginId) -> Option<PluginStatus> {
        self.handle().status(id)
    }

    pub fn plugins(&self) -> Vec<(PluginId, PluginStatus)> {
        self.handle().plugins()
    }

    /// Stops every plugin in reverse activation order and rejects further
    /// lifecycle operations.
    pub async fn shutdown(&self) -> Result<(), KernelError> {
        if in_lifecycle() {
            return Err(KernelError::Reentrant);
        }
        let mut shut_down = self.inner.ops.lock().await;
        if *shut_down {
            return Ok(());
        }
        *shut_down = true;
        loop {
            let last = {
                let state = self.inner.state();
                state
                    .plugins
                    .iter()
                    .filter_map(|(id, e)| e.active.as_ref().map(|a| (a.activation_seq, id)))
                    .max()
                    .map(|(_, id)| id.clone())
            };
            let Some(id) = last else { break };
            stop_cascade(&self.inner, &id).await;
        }
        let mut state = self.inner.state();
        state.plugins.clear();
        state.services.clear();
        state.hooks.clear();
        for list in state.handlers.values_mut() {
            list.retain(|h| matches!(h.owner, Owner::Subscription(_)));
        }
        Ok(())
    }
}

/// Weak, cloneable access to a kernel for plugins and hosts.
#[derive(Clone)]
pub struct KernelHandle {
    inner: Weak<Inner>,
}

impl KernelHandle {
    pub(crate) fn from_inner(inner: &Arc<Inner>) -> Self {
        Self {
            inner: Arc::downgrade(inner),
        }
    }

    fn upgrade(&self) -> Result<Arc<Inner>, KernelError> {
        self.inner.upgrade().ok_or(KernelError::ShutDown)
    }

    pub async fn mount(
        &self,
        plugin: Arc<dyn Plugin>,
        options: MountOptions,
    ) -> Result<PluginStatus, KernelError> {
        mount(&self.upgrade()?, plugin, options).await
    }

    pub async fn unmount(&self, id: &PluginId) -> Result<(), KernelError> {
        unmount(&self.upgrade()?, id).await
    }

    pub async fn update_config(
        &self,
        id: &PluginId,
        config: serde_json::Value,
    ) -> Result<PluginStatus, KernelError> {
        restart(&self.upgrade()?, id, Some(config)).await
    }

    pub async fn restart(&self, id: &PluginId) -> Result<PluginStatus, KernelError> {
        restart(&self.upgrade()?, id, None).await
    }

    pub fn status(&self, id: &PluginId) -> Option<PluginStatus> {
        let inner = self.inner.upgrade()?;
        let state = inner.state();
        state.plugins.get(id).map(|e| e.status.clone())
    }

    pub fn plugins(&self) -> Vec<(PluginId, PluginStatus)> {
        let Some(inner) = self.inner.upgrade() else {
            return Vec::new();
        };
        let state = inner.state();
        state
            .plugins
            .iter()
            .map(|(id, e)| (id.clone(), e.status.clone()))
            .collect()
    }

    /// Looks up the default (earliest-registered) instance of a service.
    pub fn get<S: ?Sized + Send + Sync + 'static>(&self, key: ServiceKey<S>) -> Option<Arc<S>> {
        self.lookup(key, None)
    }

    pub fn get_instance<S: ?Sized + Send + Sync + 'static>(
        &self,
        key: ServiceKey<S>,
        instance: &str,
    ) -> Option<Arc<S>> {
        self.lookup(key, Some(&instance.to_owned()))
    }

    /// Every registered instance of a service, oldest first, as
    /// `(instance, service)` pairs. Lookups are not bindings: callers that
    /// resolve per use are unaffected when providers come and go.
    pub fn get_all<S: ?Sized + Send + Sync + 'static>(
        &self,
        key: ServiceKey<S>,
    ) -> Vec<(String, Arc<S>)> {
        let Some(inner) = self.inner.upgrade() else {
            return Vec::new();
        };
        let state = inner.state();
        let name = key.name();
        let mut entries: Vec<&ServiceEntry> =
            state.services.get(&name).into_iter().flatten().collect();
        entries.sort_by_key(|e| e.seq);
        entries
            .into_iter()
            .filter_map(|e| Some((e.instance.clone(), e.get::<S>(&name).ok()?)))
            .collect()
    }

    /// Whether any handler is registered for hook `H`.
    pub fn has_handlers<H: 'static>(&self) -> bool {
        self.inner.upgrade().is_some_and(|inner| {
            inner
                .state()
                .hooks
                .get(&TypeId::of::<H>())
                .is_some_and(|list| !list.is_empty())
        })
    }

    fn lookup<S: ?Sized + Send + Sync + 'static>(
        &self,
        key: ServiceKey<S>,
        instance: Option<&String>,
    ) -> Option<Arc<S>> {
        let inner = self.inner.upgrade()?;
        let state = inner.state();
        let name = key.name();
        resolve(state.services.get(&name), instance)?
            .get::<S>(&name)
            .ok()
    }

    /// Emits an event to every handler of type `E`. Handler panics are
    /// contained and do not stop other handlers.
    pub fn emit<E: Event>(&self, event: &E) {
        if let Some(inner) = self.inner.upgrade() {
            emit(&inner, event);
        }
    }

    /// Subscribes the host to events of type `E` until the returned guard is
    /// dropped.
    pub fn subscribe<E: Event>(
        &self,
        handler: impl Fn(&E) + Send + Sync + 'static,
    ) -> Subscription {
        let type_id = TypeId::of::<E>();
        let Some(inner) = self.inner.upgrade() else {
            return Subscription {
                kernel: Weak::new(),
                type_id,
                id: 0,
            };
        };
        let id = inner.next_seq();
        let handler: HandlerFn<E> = Arc::new(handler);
        inner
            .state()
            .handlers
            .entry(type_id)
            .or_default()
            .push(HandlerEntry {
                owner: Owner::Subscription(id),
                handler: Arc::new(handler),
            });
        Subscription {
            kernel: self.inner.clone(),
            type_id,
            id,
        }
    }

    fn handlers<F: Clone + 'static>(&self, hook: TypeId) -> Result<Vec<(PluginId, F)>, HookError> {
        let inner = self.inner.upgrade().ok_or(HookError::KernelGone)?;
        let state = inner.state();
        Ok(state
            .hooks
            .get(&hook)
            .into_iter()
            .flatten()
            .filter_map(|h| Some((h.owner.clone(), h.handler.downcast_ref::<F>()?.clone())))
            .collect())
    }

    /// Runs handlers in priority order until one returns `Some`. A panicking
    /// handler aborts the chain with [`HookError::Panicked`]; security callers
    /// must treat that as a denial.
    pub async fn bail<H: BailHook>(&self, input: H::Input) -> Result<Option<H::Output>, HookError> {
        for (owner, handler) in self.handlers::<BailFn<H>>(TypeId::of::<H>())? {
            let input = input.clone();
            match guarded(|| handler(input)).await {
                Ok(Some(output)) => return Ok(Some(output)),
                Ok(None) => {}
                Err(()) => return Err(panicked(H::NAME, owner)),
            }
        }
        Ok(None)
    }

    /// Passes a value through every handler in priority order.
    pub async fn waterfall<H: WaterfallHook>(
        &self,
        value: H::Value,
    ) -> Result<H::Value, HookError> {
        let mut value = value;
        for (owner, handler) in self.handlers::<WaterfallFn<H>>(TypeId::of::<H>())? {
            let input = value.clone();
            value = guarded(|| handler(input))
                .await
                .map_err(|()| panicked(H::NAME, owner))?;
        }
        Ok(value)
    }

    /// Runs every handler sequentially in priority order.
    pub async fn serial<H: SerialHook>(&self, input: H::Input) -> Result<(), HookError> {
        for (owner, handler) in self.handlers::<SerialFn<H>>(TypeId::of::<H>())? {
            let input = input.clone();
            guarded(|| handler(input))
                .await
                .map_err(|()| panicked(H::NAME, owner))?;
        }
        Ok(())
    }
}

fn panicked(hook: &'static str, owner: PluginId) -> HookError {
    HookError::Panicked {
        hook,
        owner: owner.to_string(),
    }
}

fn emit<E: Event>(inner: &Inner, event: &E) {
    let handlers: Vec<HandlerFn<E>> = {
        let state = inner.state();
        state
            .handlers
            .get(&TypeId::of::<E>())
            .into_iter()
            .flatten()
            .filter_map(|h| h.handler.downcast_ref::<HandlerFn<E>>().cloned())
            .collect()
    };
    for handler in handlers {
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| handler(event)));
    }
}

fn emit_changes(inner: &Inner, changes: Changes) {
    for change in changes {
        emit(inner, &change);
    }
}

async fn mount(
    inner: &Arc<Inner>,
    plugin: Arc<dyn Plugin>,
    options: MountOptions,
) -> Result<PluginStatus, KernelError> {
    if in_lifecycle() {
        return Err(KernelError::Reentrant);
    }
    let shut_down = inner.ops.lock().await;
    if *shut_down {
        return Err(KernelError::ShutDown);
    }
    let manifest = plugin.manifest().clone();
    manifest.validate(&options)?;
    let id = manifest.id.clone();
    {
        let mut state = inner.state();
        if state.plugins.contains_key(&id) {
            return Err(KernelError::DuplicatePlugin(id));
        }
        state.plugins.insert(
            id.clone(),
            Entry {
                plugin,
                manifest,
                options,
                status: PluginStatus::Pending {
                    missing: Vec::new(),
                },
                mount_seq: inner.next_seq(),
                active: None,
            },
        );
    }
    reconcile(inner).await;
    drop(shut_down);
    status_of(inner, &id)
}

async fn unmount(inner: &Arc<Inner>, id: &PluginId) -> Result<(), KernelError> {
    if in_lifecycle() {
        return Err(KernelError::Reentrant);
    }
    let _ops = inner.ops.lock().await;
    if !inner.state().plugins.contains_key(id) {
        return Err(KernelError::UnknownPlugin(id.clone()));
    }
    stop_cascade(inner, id).await;
    inner.state().plugins.remove(id);
    reconcile(inner).await;
    Ok(())
}

async fn restart(
    inner: &Arc<Inner>,
    id: &PluginId,
    config: Option<serde_json::Value>,
) -> Result<PluginStatus, KernelError> {
    if in_lifecycle() {
        return Err(KernelError::Reentrant);
    }
    let shut_down = inner.ops.lock().await;
    if *shut_down {
        return Err(KernelError::ShutDown);
    }
    if !inner.state().plugins.contains_key(id) {
        return Err(KernelError::UnknownPlugin(id.clone()));
    }
    stop_cascade(inner, id).await;
    let mut changes = Changes::new();
    if let Some(entry) = inner.state().plugins.get_mut(id) {
        if let Some(config) = config {
            entry.options.config = config;
        }
        let pending = PluginStatus::Pending {
            missing: Vec::new(),
        };
        set_status(id, entry, pending, &mut changes);
    }
    emit_changes(inner, changes);
    reconcile(inner).await;
    drop(shut_down);
    status_of(inner, id)
}

fn status_of(inner: &Inner, id: &PluginId) -> Result<PluginStatus, KernelError> {
    inner
        .state()
        .plugins
        .get(id)
        .map(|e| e.status.clone())
        .ok_or_else(|| KernelError::UnknownPlugin(id.clone()))
}

/// Starts every waiting plugin whose requirements resolve, in mount order,
/// until no more can start; then refreshes the waiting statuses.
async fn reconcile(inner: &Arc<Inner>) {
    loop {
        let next = {
            let state = inner.state();
            state
                .plugins
                .iter()
                .filter(|(_, e)| e.waiting() && missing(&state, e).is_empty())
                .min_by_key(|(_, e)| e.mount_seq)
                .map(|(id, _)| id.clone())
        };
        let Some(id) = next else { break };
        start(inner, &id).await;
    }
    let mut changes = Changes::new();
    refresh_waiting(&mut inner.state(), &mut changes);
    emit_changes(inner, changes);
}

async fn start(inner: &Arc<Inner>, id: &PluginId) {
    let mut changes = Changes::new();
    let (plugin, manifest, options) = {
        let mut state = inner.state();
        let Some(entry) = state.plugins.get_mut(id) else {
            return;
        };
        set_status(id, entry, PluginStatus::Starting, &mut changes);
        (
            entry.plugin.clone(),
            entry.manifest.clone(),
            entry.options.clone(),
        )
    };
    emit_changes(inner, changes);

    let mut scope = Scope::new(inner.clone(), manifest, options);
    let outcome = LIFECYCLE
        .scope((), async {
            let (plugin, scope) = (plugin.as_ref(), &mut scope);
            let applied = std::panic::catch_unwind(AssertUnwindSafe(move || {
                let scope = scope;
                plugin.apply(scope)
            }));
            match applied {
                Ok(future) => match AssertUnwindSafe(future).catch_unwind().await {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(error)) => Err(error.to_string()),
                    Err(_) => Err("apply panicked".to_owned()),
                },
                Err(_) => Err("apply panicked".to_owned()),
            }
        })
        .await;
    let Scope {
        staged, runtime, ..
    } = scope;

    let mut changes = Changes::new();
    let failed = {
        let mut state = inner.state();
        match outcome {
            Ok(()) if state.plugins.contains_key(id) => {
                for (name, entry) in staged.services {
                    state.services.entry(name).or_default().push(entry);
                }
                for (hook, entry) in staged.hooks {
                    insert_sorted(state.hooks.entry(hook).or_default(), entry);
                }
                for (event, entry) in staged.handlers {
                    state.handlers.entry(event).or_default().push(entry);
                }
                if let Some(entry) = state.plugins.get_mut(id) {
                    entry.active = Some(Active {
                        activation_seq: inner.next_seq(),
                        bindings: staged.bindings,
                        runtime,
                    });
                    set_status(id, entry, PluginStatus::Active, &mut changes);
                }
                None
            }
            Ok(()) => Some((runtime, None)),
            Err(message) => Some((runtime, Some(message))),
        }
    };
    if let Some((runtime, message)) = failed {
        LIFECYCLE.scope((), runtime.dispose()).await;
        if let Some(message) = message
            && let Some(entry) = inner.state().plugins.get_mut(id)
        {
            set_status(id, entry, PluginStatus::Failed { message }, &mut changes);
        }
    }
    emit_changes(inner, changes);
}

/// Stops `root` and every active plugin transitively bound to it. Their
/// registrations are withdrawn first; scopes are then disposed in reverse
/// activation order (dependents before providers). Stopped dependents become
/// `Pending`; the caller decides the root's next state.
async fn stop_cascade(inner: &Arc<Inner>, root: &PluginId) {
    let mut changes = Changes::new();
    let (stopped, victims) = {
        let mut state = inner.state();
        if state.plugins.get(root).is_none_or(|e| e.active.is_none()) {
            return;
        }
        let mut stopped = BTreeSet::from([root.clone()]);
        loop {
            let added: Vec<PluginId> = state
                .plugins
                .iter()
                .filter(|(id, e)| {
                    !stopped.contains(*id)
                        && e.active.as_ref().is_some_and(|a| {
                            a.bindings.iter().any(|b| stopped.contains(&b.provider))
                        })
                })
                .map(|(id, _)| id.clone())
                .collect();
            if added.is_empty() {
                break;
            }
            stopped.extend(added);
        }
        for entries in state.services.values_mut() {
            entries.retain(|e| !stopped.contains(&e.provider));
        }
        state.services.retain(|_, entries| !entries.is_empty());
        for list in state.hooks.values_mut() {
            list.retain(|h| !stopped.contains(&h.owner));
        }
        for list in state.handlers.values_mut() {
            list.retain(|h| !matches!(&h.owner, Owner::Plugin(id) if stopped.contains(id)));
        }
        let mut victims = Vec::new();
        for id in &stopped {
            if let Some(entry) = state.plugins.get_mut(id)
                && let Some(active) = entry.active.take()
            {
                set_status(id, entry, PluginStatus::Stopping, &mut changes);
                victims.push((active.activation_seq, active.runtime));
            }
        }
        victims.sort_by_key(|victim| std::cmp::Reverse(victim.0));
        (stopped, victims)
    };
    emit_changes(inner, changes);
    for (_, runtime) in victims {
        LIFECYCLE.scope((), runtime.dispose()).await;
    }
    let mut changes = Changes::new();
    {
        let mut state = inner.state();
        for id in stopped.iter().filter(|id| *id != root) {
            if let Some(entry) = state.plugins.get_mut(id) {
                let pending = PluginStatus::Pending {
                    missing: Vec::new(),
                };
                set_status(id, entry, pending, &mut changes);
            }
        }
    }
    emit_changes(inner, changes);
}

/// Recomputes `Pending { missing }` / `DependencyCycle` for waiting plugins.
/// A waiting plugin is in a cycle if it transitively waits on itself through
/// other waiting plugins that declare the missing services.
fn refresh_waiting(state: &mut State, changes: &mut Changes) {
    let waiting: Vec<(PluginId, Vec<ServiceName>)> = state
        .plugins
        .iter()
        .filter(|(_, e)| e.waiting())
        .map(|(id, e)| (id.clone(), missing(state, e)))
        .collect();
    let edges: BTreeMap<PluginId, Vec<PluginId>> = waiting
        .iter()
        .map(|(id, needs)| {
            let targets = waiting
                .iter()
                .filter(|(other, _)| {
                    state
                        .plugins
                        .get(other)
                        .is_some_and(|e| needs.iter().any(|s| e.manifest.provides.contains(s)))
                })
                .map(|(other, _)| other.clone())
                .collect();
            (id.clone(), targets)
        })
        .collect();
    for (id, needs) in waiting {
        let cycle = cycle_through(&edges, &id);
        let status = if cycle.is_empty() {
            PluginStatus::Pending { missing: needs }
        } else {
            PluginStatus::DependencyCycle { cycle }
        };
        if let Some(entry) = state.plugins.get_mut(&id) {
            set_status(&id, entry, status, changes);
        }
    }
}

fn reachable(edges: &BTreeMap<PluginId, Vec<PluginId>>, from: &PluginId) -> BTreeSet<PluginId> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<&PluginId> = edges.get(from).into_iter().flatten().collect();
    while let Some(next) = stack.pop() {
        if seen.insert(next.clone()) {
            stack.extend(edges.get(next).into_iter().flatten());
        }
    }
    seen
}

fn cycle_through(edges: &BTreeMap<PluginId, Vec<PluginId>>, id: &PluginId) -> Vec<PluginId> {
    let forward = reachable(edges, id);
    if !forward.contains(id) {
        return Vec::new();
    }
    forward
        .into_iter()
        .filter(|n| n == id || reachable(edges, n).contains(id))
        .collect()
}
