use std::{any::Any, cmp::Reverse, sync::Arc};

use crate::{BoxFuture, PluginId};

/// A hook whose handlers run in priority order until one returns `Some`.
/// Typical use: authorization decisions (`tool.before_invoke`).
pub trait BailHook: 'static {
    const NAME: &'static str;
    type Input: Clone + Send + 'static;
    type Output: Send + 'static;
}

/// A hook whose handlers each transform a value in priority order.
/// Typical use: request rewriting (`model.before_request`).
pub trait WaterfallHook: 'static {
    const NAME: &'static str;
    type Value: Clone + Send + 'static;
}

/// A hook whose handlers all run sequentially in priority order.
/// Typical use: lifecycle notifications that must complete in order.
pub trait SerialHook: 'static {
    const NAME: &'static str;
    type Input: Clone + Send + 'static;
}

pub(crate) type BailFn<H> =
    Arc<dyn Fn(<H as BailHook>::Input) -> BoxFuture<Option<<H as BailHook>::Output>> + Send + Sync>;
pub(crate) type WaterfallFn<H> = Arc<
    dyn Fn(<H as WaterfallHook>::Value) -> BoxFuture<<H as WaterfallHook>::Value> + Send + Sync,
>;
pub(crate) type SerialFn<H> = Arc<dyn Fn(<H as SerialHook>::Input) -> BoxFuture<()> + Send + Sync>;

/// A registered handler; `handler` holds one of the `*Fn<H>` aliases.
#[derive(Clone)]
pub(crate) struct HookEntry {
    pub owner: PluginId,
    pub priority: i32,
    pub seq: u64,
    pub handler: Arc<dyn Any + Send + Sync>,
}

/// Inserts keeping higher priority first, then registration order.
pub(crate) fn insert_sorted(list: &mut Vec<HookEntry>, entry: HookEntry) {
    let key = |h: &HookEntry| (Reverse(h.priority), h.seq);
    let position = list.partition_point(|h| key(h) <= key(&entry));
    list.insert(position, entry);
}
