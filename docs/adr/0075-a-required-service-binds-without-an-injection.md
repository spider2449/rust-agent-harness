# ADR 0075: A Required Service Binds Without An Injection

Status: Accepted

Date: 2026-11-10

## Context

A plugin starts when every service in its `requires` resolves (ADR 0036).
It was stopped again only through a *binding*, which `Scope::inject` records
in `apply`. A plugin that lists a service in `requires` and looks it up later
with `KernelHandle::get` had no binding. When the last provider of that
service stopped, the plugin stayed `Active` although the condition it was
started under no longer held, and its lookups returned nothing.

So `requires` meant two things: a condition for starting, always, and a
condition for running, only after an injection. No plugin in this repository
relies on the difference: every one that requires a service injects it.

## Decision

1. **A plugin runs only while every service it requires resolves.** When a
   stop would leave an active plugin without one, that plugin is part of the
   same cascade: it is stopped before the provider, becomes `Pending` with the
   service listed as missing, and starts again once the service resolves.
2. **Resolving is what starting uses.** With a mount `binding` for the
   service, the bound instance must remain. Without one, any provider will
   do: a plugin that looks a service up when it needs it is not touched while
   another provider is left.
3. **Injection is unchanged.** It still binds the plugin to the provider it
   got, also for `optional` services, and also while other providers remain,
   since the plugin holds that provider's object.
4. **`optional` services stay unbound** unless injected.

## Consequences

- Restarting or replacing the only provider of a service restarts the
  plugins that require it, whether they inject it or not.
- A plugin that wants to survive the loss of a service must list it as
  `optional` and look it up when needed.
- Process and WASM plugins already inject what they require; nothing changes
  for them.
- The cascade does a little more work per stop: one pass over `requires` of
  the active plugins.
