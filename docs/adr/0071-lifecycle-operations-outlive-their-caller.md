# ADR 0071: Lifecycle Operations Outlive Their Caller

Status: Accepted

Date: 2026-11-10

## Context

`mount`, `unmount`, `restart`, `update_config` and `shutdown` change kernel
state in several steps with awaits between them: registrations are withdrawn,
scopes are disposed, `apply` runs, registrations are committed. The steps ran
inside the caller's future. A caller that was dropped between two steps left
the operation half done: the plugin and the dependents stopped with it stayed
`Stopping` (or `Starting`) with their services, hooks and handlers withdrawn,
their dispose effects never ran, and nothing started them again, because only
`Pending` plugins are reconciled.

Callers are dropped in normal use. `frontend-web` awaits these operations in
HTTP handlers, and a handler ends when its client disconnects, for example a
browser tab closed during a settings save. For a plugin that guards tool calls
with a `tool.before_invoke` hook, the result was a missing guard.

## Decision

1. **Each lifecycle operation runs as a task of its own.** `Kernel` and
   `KernelHandle` start the operation as a task and wait for it. Dropping the
   caller's future no longer stops the operation; it runs to its normal end.
2. **The task is owned by the kernel's operation order.** It holds the
   operations lock like before, so operations stay serialized and `shutdown`
   waits for one that was abandoned. The task holds the kernel state alive
   until it ends.
3. **Re-entrancy is still refused.** The check runs in the caller, before the
   task starts, so a lifecycle call from inside `apply` or a dispose effect
   returns `KernelError::Reentrant` as before.
4. **Results are unchanged.** A panic inside an operation resumes in the
   caller. If the runtime shuts down and drops the task, the caller gets
   `KernelError::ShutDown`.

## Consequences

- Lifecycle operations need a Tokio runtime, which scopes already required
  for their tasks.
- The public kernel API, `rah-api` and `rah-protocol` are unchanged.
- A caller cannot cancel a lifecycle operation by dropping it. An `apply` or
  dispose effect that never returns still blocks later operations; bounding
  those is a separate problem (see `docs/KNOWN_ISSUES.md`).
