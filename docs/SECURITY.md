# RAH Security Model — Plugin Kernel

Governing decision: [ADR 0036](adr/0036-plugin-kernel-everything-is-a-plugin.md),
section 4. The authority-first security model of v0.35.0 and earlier is
documented in [docs/legacy/SECURITY.md](legacy/SECURITY.md).

## Trust boundary

**Mounting is trust; model output is never trusted.**

| Source | Trust |
|---|---|
| Plugins mounted by the user's configuration (static or process) | Trusted code, comparable to a dependency |
| Process plugins calling host services | Limited to the services in their own `requires`, enforced by the host |
| WASM plugins (planned) | Capability-isolated |
| Model output, tool-call arguments, provider and tool metadata | Untrusted requests |
| Frontend presentation | Grants nothing by itself; Ask confirmations come from the user |

## Invariants

1. Every model-originated tool call passes through `ToolInvoker` and the
   `tool.before_invoke` hook chain. No loop or model plugin calls tool providers
   directly.
2. Under the `default` preset, `policy-authority` decides Allow, Deny or Ask.
   A missing permission assignment fails closed. Provider or tool metadata
   cannot grant or escalate permission.
3. The `minimal` preset runs without a policy plugin and must print a prominent
   *unguarded* warning at startup.
4. Uncertain external effects are never replayed automatically. Timeout,
   cancellation, disconnect, crash or a lost response does not imply rollback.
   Session replay never re-executes tool calls.
5. Shell and process execution exist only as opt-in plugins that require a
   `sandbox` service. They use a fixed program and argument vector, explicit
   cwd and environment policy, bounded output, timeout and cancellation. Under
   `policy-authority` they default to Ask.
6. Process supervision is not OS sandboxing. RAH makes no network-isolation,
   rollback or strong-sandbox claim until one is implemented and tested.
7. Credentials are backend configuration (environment or a storage plugin).
   They never come from model output and never travel through frontend
   channels.

## Transitional note

Until phase P4 delivers `policy-authority` and `ToolInvoker`, the legacy
crates on `master` keep their v0.35.0 behavior. Model tool requests still go
through the legacy host port, permission dispatch and `ToolRegistry`, as
documented in the legacy security model.
