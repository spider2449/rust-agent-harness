# ADR 0049: Tool Input Repair Runs Before the Policy Sees the Call

Status: Accepted

Date: 2026-10-17

## Context

Models, small ones especially, send arguments that are one step away from
valid: a number as a string (`"limit": "20"`), an object as a JSON string, a
`null` for an optional field. The tool then fails with `invalid input` and
the model has to spend a round to retry. opencode repairs these cases in a
hook when the tool's input schema makes the fix unambiguous.

RAH's `tool.before_invoke` is a bail hook that answers allow, deny or ask. It
cannot rewrite the call, and it should not: a handler that both rewrites and
authorizes makes the decision depend on handler order.

## Decision

1. **A new waterfall hook, `tool.repair_input`** (`rah_api::hooks::ToolInputRepair`),
   whose value is the `ToolRequestContext` of the call. A handler may change
   only `call.input`; the invoker keeps the call id, the tool name and the
   definition it found itself.
2. **`ToolInvoker::invoke` runs it first**, before `tool.before_invoke`, so
   the policy, the approver and the admission providers see the input the
   tool will receive. A handler that fails (panics) leaves the input as the
   model sent it; repair is a convenience, never a reason to refuse a call.
   The input is repaired once, however many handlers there are.
3. **The plugin `tool-input-repair`** is the only handler RAH ships. It
   repairs, by the tool's `input_schema`, only what the schema makes
   unambiguous: a JSON string for an object or array, a numeric or boolean
   string, a lone value for a one-item array, `null` or `{}` for an optional
   field that cannot be either, fields a closed object does not declare,
   tuples, typed dictionaries, nested fields and local `$ref`s (to a depth of
   6). A value that does not fit stays as it is, and the tool's own
   validation reports it. It has no configuration and requires no service.
4. **Presets mount it.** It cannot grant anything: the policy decides on the
   repaired input as it would on input the model had written correctly.
5. **Not offered to process or WASM plugins** as a hook for now: the hook
   lists of ADR 0039 are unchanged.

## Consequences

- The session log keeps the call as the model requested it; the tool
  receives, and the policy judged, the repaired input. The difference shows
  only in the tool's own output.
- A tool whose schema is wrong or too loose can receive input the model did
  not send in exactly that form. The repairs never add a field, only convert,
  unwrap or drop.
- Another plugin can attach to the same hook (for example to fill defaults)
  with the same limits: it runs before the policy and cannot change which
  tool is called.
