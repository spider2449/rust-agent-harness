# ADR 0076: Disposal Is Bounded

Status: Accepted

Date: 2026-11-10

## Context

When a plugin stops, the kernel disposes its scope: it cancels the scope's
tasks, waits until each has reached an await point, and runs the dispose
effects (ADR 0036). It waited without a limit, and it waits while holding
the operations lock (ADR 0071). One task stuck in synchronous work, or one
dispose effect that never returns, therefore stopped every later mount,
unmount, restart and configuration change, and the shutdown with them. ADR
0071 named this and left it open.

Mounted plugins are trusted code, so this is not a defence against a hostile
plugin. It is about a fault in one plugin (a blocking call on a dead socket,
a child that does not answer) taking the whole host with it.

## Decision

1. **The kernel waits a limited time for a scope.** For the owned tasks
   together, and for each dispose effect, at most the kernel's limit:
   10 seconds for `Kernel::new`, or the value given to
   `Kernel::with_dispose_limit`. Disposal within the limit is as before.
2. **After the limit the kernel goes on.** Tasks that did not end are
   aborted: they stop at their next await point, as the cancelled scope
   already asked. An effect that did not return is aborted the same way and
   the next effect runs. The plugin's registrations were withdrawn before the
   disposal started, so nothing new reaches what is left behind.
3. **Each dispose effect runs as a task of its own,** so that an effect
   stuck in synchronous work blocks one runtime thread and not the lifecycle
   operation. Effects still run one after the other, in reverse registration
   order, and lifecycle calls from inside one are still `Reentrant`.
4. **The limit is timed by a thread, not by the runtime's timer.** The work
   being bounded may be synchronous work on the threads that drive that
   timer; the timer then never fires. The thread lives only for the wait.
5. **The kernel reports it.** It emits `DisposeOverrun { plugin, tasks,
   effects }` with what it did not wait for. The hosts turn it into a
   `HostNotice` (ADR 0057), so it is printed and shown in the Web UI.
6. **`apply` is not bounded.** A plugin may take long to start for a good
   reason (a server handshake, labeling a large tree). An `apply` that never
   returns still blocks later operations.

## Consequences

- "Nothing outlives its owner" now has a stated exception: after an
  overrun, a task in synchronous work or an effect may run on until its next
  await point, and cleanup may be incomplete (a child process not asked to
  shut down, a directory not removed). It is reported, not silent.
- Synchronous work cannot be interrupted. It keeps its runtime thread until
  it returns; on a single-threaded runtime the kernel cannot go on at all.
  `rah` and the desktop shell run a multi-threaded runtime.
- A dispose effect that legitimately needs more than 10 seconds is cut off.
  None in this repository comes close: they close a connection, ask a child
  to shut down with a 500 ms limit, or remove a temporary directory.
- The kernel starts one short-lived thread per wait. Stopping plugins is
  rare, so this costs little.
