use std::{
    any::{Any, TypeId},
    sync::{Arc, Weak},
};

use crate::{PluginId, PluginStatus, kernel::Inner};

/// Any `Send + Sync + 'static` value can be emitted as an event. Handlers run
/// synchronously in registration order; they must be quick and should hand
/// long work to a [`crate::Spawner`].
pub trait Event: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Event for T {}

/// Emitted by the kernel whenever a plugin changes lifecycle state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginStatusChanged {
    pub plugin: PluginId,
    pub status: PluginStatus,
}

pub(crate) type HandlerFn<E> = Arc<dyn Fn(&E) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Plugin(PluginId),
    Subscription(u64),
}

#[derive(Clone)]
pub(crate) struct HandlerEntry {
    pub owner: Owner,
    pub handler: Arc<dyn Any + Send + Sync>,
}

/// Host-level event subscription. Dropping it unsubscribes.
#[must_use = "dropping a Subscription unsubscribes immediately"]
pub struct Subscription {
    pub(crate) kernel: Weak<Inner>,
    pub(crate) type_id: TypeId,
    pub(crate) id: u64,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(inner) = self.kernel.upgrade() {
            let mut state = inner.state();
            if let Some(list) = state.handlers.get_mut(&self.type_id) {
                list.retain(|h| h.owner != Owner::Subscription(self.id));
            }
        }
    }
}
