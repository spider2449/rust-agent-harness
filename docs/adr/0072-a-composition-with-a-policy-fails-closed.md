# ADR 0072: A Composition With a Policy Fails Closed

Status: Accepted

Date: 2026-11-10

## Context

Tool calls are authorized by the `tool.before_invoke` hook chain (ADR 0036).
The `ToolInvoker` allowed a call that no handler decided, so that the
`minimal` preset can run without a policy, behind an `UNGUARDED` warning.

The same rule applied whenever the policy's handler was absent for a moment
or by accident, in a composition that was meant to be guarded:

- while `policy-authority` restarts (a saved Permissions form, "allow this
  command from now on" during a running turn, a feature switch), its handler
  is withdrawn and registered again, and a call decided in between was
  allowed without a check;
- a policy that failed, or was left stopped, allowed every call;
- a plugin mounted before the policy could run a turn before it existed.

The boot already refuses to continue without its policy (ADR 0059), but
nothing covered the time after the boot.

## Decision

1. **A `tool-guard` service states that a policy is expected.** `rah-api`
   defines `tools::ToolGuard` with `required()`. While any mounted provider
   requires a policy, the `ToolInvoker` denies a call that no
   `tool.before_invoke` handler decided ("no tool policy is active"). Its
   effect is `none`; nothing is started. Without a provider, or when none
   requires a policy, an undecided call is allowed as before.
2. **The distribution provides it.** The `distribution` plugin is mounted
   first and cannot be changed through the admin service. It requires a
   policy from before the first plugin of a distribution that lists a policy
   kind is mounted, so boot order no longer matters.
3. **Only the user ends the requirement.** Unmounting the last policy through
   the admin service (the Plugins panel, behind its warning) is the decision
   to run unguarded, and ends the requirement; mounting a policy there starts
   it again. A restart, a reconfiguration, a failure or an unmount that did
   not come through the admin service leaves it in force.
4. **Frontends can tell the two states apart.** `ToolInvoker::guard_required`
   and `guard_required` in `GET /api/status` say that a policy is expected;
   the Web UI shows "NO POLICY: tools denied" instead of `UNGUARDED` then.

## Consequences

- A policy handler that leaves a decision open (answers nothing) now denies
  in a guarded composition. `policy-authority` always decides, so the
  presets behave as before.
- A plugin that only adds a second `tool.before_invoke` handler cannot keep
  tools running when the policy is gone.
- The kernel and `rah-protocol` are unchanged. `rah-api` gains the service
  key, the trait and `ToolInvoker::guard_required`.
- The `minimal` preset and a configuration without a policy are unchanged:
  no guard is required and `rah` warns `UNGUARDED`.
