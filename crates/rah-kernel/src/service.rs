use std::{
    any::{Any, TypeId, type_name},
    fmt,
    marker::PhantomData,
    sync::Arc,
};

use crate::{PluginId, ScopeError, ServiceName};

/// Typed key of a service contract. Keys with the same name must use the same
/// type; a mismatch is reported as [`ScopeError::TypeMismatch`].
///
/// ```
/// trait Clock: Send + Sync { fn now(&self) -> u64; }
/// const CLOCK: rah_kernel::ServiceKey<dyn Clock> = rah_kernel::ServiceKey::new("clock");
/// ```
pub struct ServiceKey<S: ?Sized + 'static> {
    name: &'static str,
    _marker: PhantomData<fn() -> Arc<S>>,
}

impl<S: ?Sized + 'static> ServiceKey<S> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _marker: PhantomData,
        }
    }

    pub fn name(&self) -> ServiceName {
        ServiceName::new(self.name)
    }
}

impl<S: ?Sized + 'static> Clone for ServiceKey<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ?Sized + 'static> Copy for ServiceKey<S> {}

impl<S: ?Sized + 'static> fmt::Debug for ServiceKey<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServiceKey<{}>({})", type_name::<S>(), self.name)
    }
}

/// One registered implementation of a service.
#[derive(Clone)]
pub(crate) struct ServiceEntry {
    pub instance: String,
    pub provider: PluginId,
    pub seq: u64,
    type_id: TypeId,
    type_name: &'static str,
    value: Arc<dyn Any + Send + Sync>,
}

impl ServiceEntry {
    pub fn new<S: ?Sized + Send + Sync + 'static>(
        instance: String,
        provider: PluginId,
        seq: u64,
        value: Arc<S>,
    ) -> Self {
        Self {
            instance,
            provider,
            seq,
            type_id: TypeId::of::<Arc<S>>(),
            type_name: type_name::<S>(),
            value: Arc::new(value),
        }
    }

    pub fn same_type(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }

    pub fn get<S: ?Sized + Send + Sync + 'static>(
        &self,
        service: &ServiceName,
    ) -> Result<Arc<S>, ScopeError> {
        self.value
            .downcast_ref::<Arc<S>>()
            .cloned()
            .ok_or_else(|| ScopeError::TypeMismatch {
                service: service.clone(),
                registered: self.type_name,
                requested: type_name::<S>(),
            })
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }
}

/// Resolves a service to one entry: the bound instance if a binding is given,
/// otherwise the earliest-registered instance.
pub(crate) fn resolve<'a>(
    entries: Option<&'a Vec<ServiceEntry>>,
    binding: Option<&String>,
) -> Option<&'a ServiceEntry> {
    let entries = entries?;
    match binding {
        Some(instance) => entries.iter().find(|e| &e.instance == instance),
        None => entries.iter().min_by_key(|e| e.seq),
    }
}
